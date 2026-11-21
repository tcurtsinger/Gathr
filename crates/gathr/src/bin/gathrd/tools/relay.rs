//! The gateway to the tools on the web (`gathr::web_tools`): each enabled tool is an MCP server of
//! its own at `/mcp/tools/<id>`, admitted with a running engine's token as `/mcp` is. Gathr relays
//! the tool's list of actions and each call, adding the tool's key on the way out; the engine never
//! sees the key or the link. Nothing else of MCP is relayed: no resources, prompts or completions,
//! and nothing the tool would ask the engine (sampling, elicitation, roots).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::http::request::Parts;
use gathr::web_tools::{self, Remote};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler};

use crate::api::State;

/// Where each tool is served, `{id}` its id.
pub const PATH: &str = "/mcp/tools/{id}";

/// Each tool connected since it last changed, by id.
static REMOTES: Mutex<Option<HashMap<String, Arc<Remote>>>> = Mutex::new(None);

/// The service for `PATH`, set up as Gathr's own `/mcp`.
pub fn service(state: Arc<State>) -> StreamableHttpService<Relay, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_sse_keep_alive(None);
    let relay = move || Ok(Relay(state.clone()));
    StreamableHttpService::new(relay, Default::default(), config)
}

/// Drops the tool's connection: it reconnects with what's saved at its next use.
pub fn forget(id: &str) {
    let mut remotes = REMOTES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(remotes) = remotes.as_mut() {
        remotes.remove(id);
    }
}

#[derive(Clone)]
pub struct Relay(Arc<State>);

impl Relay {
    /// The tool the request's path names, connected.
    async fn remote(&self, context: &RequestContext<RoleServer>) -> Result<Arc<Remote>, ErrorData> {
        let parts = context.extensions.get::<Parts>();
        let path = parts.map(|p| p.uri.path()).unwrap_or_default();
        let id = path.rsplit('/').next().unwrap_or_default().to_string();
        let known = {
            let remotes = REMOTES.lock().unwrap_or_else(|e| e.into_inner());
            remotes.as_ref().and_then(|r| r.get(&id)).cloned()
        };
        if let Some(remote) = known {
            return Ok(remote);
        }
        let find = id.clone();
        let saved = self
            .0
            .writer
            .call(move |conn| {
                let tool = web_tools::get(conn, &find)?;
                Ok::<_, gathr::Error>((tool, web_tools::key(conn, &find)?))
            })
            .await;
        let (tool, key) = match saved {
            Some(Ok(saved)) => saved,
            _ => return Err(ErrorData::invalid_request("There's no such tool", None)),
        };
        if !tool.enabled {
            return Err(ErrorData::invalid_request("This tool is disabled", None));
        }
        let remote = web_tools::connect(&tool.link, key.as_deref(), web_tools::CONNECT)
            .await
            .map_err(|_| ErrorData::internal_error("The tool can't be reached", None))?;
        let remote = Arc::new(remote);
        let mut known = REMOTES.lock().unwrap_or_else(|e| e.into_inner());
        known
            .get_or_insert_with(HashMap::new)
            .insert(id, remote.clone());
        Ok(remote)
    }
}

/// A failure drops the connection, so the next call starts a new one.
fn kept<T>(
    context: &RequestContext<RoleServer>,
    done: Result<T, ErrorData>,
) -> Result<T, ErrorData> {
    if done.is_err() {
        let parts = context.extensions.get::<Parts>();
        let path = parts.map(|p| p.uri.path()).unwrap_or_default();
        forget(path.rsplit('/').next().unwrap_or_default());
    }
    done
}

impl ServerHandler for Relay {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let remote = self.remote(&context).await?;
        kept(&context, web_tools::list_page(&remote, request).await)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let remote = self.remote(&context).await?;
        kept(&context, web_tools::call(&remote, request).await)
    }
}
