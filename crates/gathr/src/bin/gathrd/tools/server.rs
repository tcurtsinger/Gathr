//! The MCP server and its tools (0005), on rmcp's Streamable HTTP service. The API's guard has
//! already checked the engine's token and put its [`Caller`] on the request.

use std::sync::Arc;
use std::time::Instant;

use axum::http::request::Parts;
use gathr::event::Event;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler, schemars, tool, tool_handler, tool_router};
use rusqlite::{Connection, OptionalExtension};

use super::Caller;
use crate::chats::Chats;

/// A message longer than this is cut, with its id to read the whole of it (0020: a tool returns
/// what the question needs, with an id for the rest).
const CUT_AT: usize = 2_000;
const DEFAULT_LIMIT: u32 = 20;
const MOST: u32 = 100;

/// The service to mount at `/mcp`. Answers are plain JSON where the call allows it, and the old
/// revisions with sessions still work (0005).
pub fn service(
    chats: Arc<Chats>,
    stopping: tokio::sync::watch::Receiver<bool>,
) -> StreamableHttpService<Tools, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_sse_keep_alive(None);
    // An older revision's session holds a `GET /mcp` stream open (Codex's does), which rmcp ends
    // only after 300 idle seconds: a stop would wait that long for it (G162).
    let (sessions, mut stop) = (config.cancellation_token.clone(), stopping.clone());
    tokio::spawn(async move {
        let _ = stop.wait_for(|stopping| *stopping).await;
        sessions.cancel();
    });
    let tools = move || {
        Ok(Tools {
            chats: chats.clone(),
            stopping: stopping.clone(),
            tool_router: Tools::tool_router()
                + Tools::coord_router()
                + Tools::person_router()
                + Tools::checks_router()
                + Tools::message_router()
                + Tools::merge_router(),
        })
    };
    StreamableHttpService::new(tools, Default::default(), config)
}

#[derive(Clone)]
pub struct Tools {
    pub(super) chats: Arc<Chats>,
    pub(super) stopping: tokio::sync::watch::Receiver<bool>,
    tool_router: ToolRouter<Self>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ReadChat {
    /// Only messages older than this message id: to page back.
    before: Option<String>,
    /// How many messages, the newest last: 20 unless set, at most 100.
    limit: Option<u32>,
    /// One message, whole, by its id: for one that came back cut.
    message: Option<String>,
}

#[tool_router]
impl Tools {
    #[tool(
        description = "Read chat history and other agents' replies, oldest first, with ids and authors. \
                       Long messages are truncated; request a message id for its full text."
    )]
    async fn read_chat(
        &self,
        Parameters(ask): Parameters<ReadChat>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let chat = caller.chat.clone();
        let read = self
            .chats
            .writer
            .call(move |conn| read(conn, &chat, &ask))
            .await
            .unwrap_or_else(|| Err("the database didn't answer".into()));
        Ok(self.respond(&caller, "read_chat", started, read).await)
    }
}

/// The engine calling, as the API's guard found it.
pub(super) fn caller_of(parts: &Parts) -> Result<Caller, ErrorData> {
    let caller = parts.extensions.get::<Caller>().cloned();
    caller.ok_or_else(|| ErrorData::internal_error("no caller", None))
}

impl Tools {
    /// A tool's answer, with its call recorded on the turn's trace; a failure says why and gives
    /// the reference that finds it, recorded before the answer goes out (0019).
    pub(super) async fn respond(
        &self,
        caller: &Caller,
        tool: &str,
        started: Instant,
        result: Result<String, String>,
    ) -> CallToolResult {
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let mut event = Event::point("gathr.tool.call")
            .trace(&trace)
            .chat(&caller.chat)
            .attr("gen_ai.tool.name", tool)
            .attr("gen_ai.agent.id", caller.agent.clone());
        event.duration_us = Some(started.elapsed().as_micros() as i64);
        match result {
            Ok(text) => {
                let event = event
                    .ok()
                    .attr("gathr.tool.result.chars", text.chars().count());
                self.chats.writer.record(event);
                CallToolResult::success(vec![ContentBlock::text(text)])
            }
            Err(why) => {
                let event = event.error(tool).attr("why", why.clone());
                self.chats.writer.record_now(event).await;
                let reference = gathr::ids::reference(&trace);
                let said = format!("{tool} failed: {why} (ref {reference})");
                CallToolResult::error(vec![ContentBlock::text(said)])
            }
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

struct Row {
    id: String,
    author: String,
    body: String,
}

fn read(conn: &Connection, chat: &str, ask: &ReadChat) -> Result<String, String> {
    let missing = |id: &str| format!("no message {id} in this chat");
    if let Some(id) = &ask.message {
        let one = rows(
            conn,
            "AND m.id = ?2",
            vec![chat.to_string().into(), id.clone().into()],
            1,
        )?;
        let row = one.first().ok_or_else(|| missing(id))?;
        return Ok(format!("{} {}: {}", row.id, row.author, row.body));
    }
    let limit = ask.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MOST) as usize;
    let mut found = match &ask.before {
        Some(id) => {
            let at: Option<i64> = conn
                .query_row(
                    "SELECT ts FROM messages WHERE id = ?1 AND chat_id = ?2",
                    [id, chat],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            let at = at.ok_or_else(|| missing(id))?;
            let params = vec![chat.to_string().into(), at.into(), id.clone().into()];
            rows(conn, "AND (m.ts, m.id) < (?2, ?3)", params, limit + 1)?
        }
        None => rows(conn, "", vec![chat.to_string().into()], limit + 1)?,
    };
    let more = found.len() > limit;
    found.truncate(limit);
    found.reverse();
    let mut lines = vec![match (found.first(), more) {
        (None, _) => "No messages.".to_string(),
        (Some(first), true) => format!("Older messages: before {}.", first.id),
        (Some(_), false) => "From the start of the chat.".to_string(),
    }];
    for row in found {
        let length = row.body.chars().count();
        let body = match length > CUT_AT {
            true => {
                let cut: String = row.body.chars().take(CUT_AT).collect();
                format!("{cut} [cut at {CUT_AT} of {length} characters]")
            }
            false => row.body,
        };
        lines.push(format!("{} {}: {}", row.id, row.author, body));
    }
    Ok(lines.join("\n"))
}

/// The chat's text messages that pass `filter`, newest first, with who wrote each.
fn rows(
    conn: &Connection,
    filter: &str,
    params: Vec<rusqlite::types::Value>,
    limit: usize,
) -> Result<Vec<Row>, String> {
    let query = format!(
        "SELECT m.id, m.role, a.name, m.body FROM messages m \
         LEFT JOIN turns t ON t.id = m.turn_id LEFT JOIN agents a ON a.id = t.agent_id \
         WHERE m.chat_id = ?1 AND m.kind = 'text' {filter} \
         ORDER BY m.ts DESC, m.id DESC LIMIT {limit}"
    );
    let row = |r: &rusqlite::Row| {
        let role: String = r.get(1)?;
        let name: Option<String> = r.get(2)?;
        let author = match role.as_str() {
            "user" => "person".to_string(),
            "gathr" => "Gathr".to_string(),
            _ => name.unwrap_or_else(|| "agent".to_string()),
        };
        Ok(Row {
            id: r.get(0)?,
            author,
            body: r.get(3)?,
        })
    };
    let run = || {
        let mut statement = conn.prepare(&query)?;
        let found = statement.query_map(rusqlite::params_from_iter(params), row)?;
        found.collect::<rusqlite::Result<Vec<Row>>>()
    };
    run().map_err(|e| e.to_string())
}
