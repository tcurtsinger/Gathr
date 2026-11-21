//! Tools that live on the web: remote MCP servers, by their link and, when one needs it, a key. Added
//! once for every agent; gathrd relays to each, so an engine never sees a tool's link's key. Gathr
//! calls out with rmcp's own client: only https, or http to this computer; a redirect is followed
//! only to the link's own origin, so the key never reaches another host; a response is capped by
//! rmcp at 16 MB.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListToolsResult, PaginatedRequestParams,
};
use rmcp::service::{ClientInitializeError, RoleClient, RunningService, ServiceError};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, InsufficientScopeError, StreamableHttpClientTransportConfig,
};
use rmcp::{ErrorData, ServiceExt};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::ids::{now_us, ulid};
use crate::{Error, fetch, seal};

/// How long a tool has to answer Gathr's hello: its connect timeout, in effect.
pub const CONNECT: Duration = Duration::from_secs(10);
/// How long a list of tools waits on each one's state, so a dead tool never holds the list.
pub const STATE: Duration = Duration::from_secs(5);
/// How long one call of a tool's action may take: some do slow work.
pub const CALL: Duration = Duration::from_secs(600);

#[derive(Debug, Serialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub link: String,
    pub enabled: bool,
    /// Whether it has a key; never the key.
    pub has_key: bool,
    pub added_ts: i64,
}

/// One thing a tool lets an agent do: its name and the first line of its description.
#[derive(Debug, Serialize)]
pub struct Action {
    pub name: String,
    pub description: Option<String>,
}

/// Why a tool didn't answer as one that works.
#[derive(Debug, PartialEq)]
pub enum Problem {
    /// It wants a key, and none was given.
    NeedsKey,
    /// The key given was refused.
    KeyRefused,
    /// It signs in through the browser (OAuth), which Gathr can't do yet.
    SignIn,
    /// Nothing that speaks MCP answered.
    NotReachable,
}

const COLUMNS: &str = "id, name, link, enabled, key IS NOT NULL, added_ts";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Tool> {
    Ok(Tool {
        id: r.get(0)?,
        name: r.get(1)?,
        link: r.get(2)?,
        enabled: r.get(3)?,
        has_key: r.get(4)?,
        added_ts: r.get(5)?,
    })
}

/// Every tool, oldest first.
pub fn list(conn: &Connection) -> Result<Vec<Tool>, Error> {
    let sql = format!("SELECT {COLUMNS} FROM tools ORDER BY added_ts, id");
    let mut statement = conn.prepare(&sql)?;
    let tools = statement.query_map([], row)?;
    Ok(tools.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Tool, Error> {
    let sql = format!("SELECT {COLUMNS} FROM tools WHERE id = ?1");
    let tool = conn.query_row(&sql, [id], row).optional()?;
    tool.ok_or_else(|| Error::Refused(format!("There's no tool {id}")))
}

/// The tool's key, unsealed; none when it has none.
pub fn key(conn: &Connection, id: &str) -> Result<Option<String>, Error> {
    let sealed: Option<Option<Vec<u8>>> = conn
        .query_row("SELECT key FROM tools WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    match sealed {
        None => Err(Error::Refused(format!("There's no tool {id}"))),
        Some(None) => Ok(None),
        Some(Some(sealed)) => seal::unseal(id, &sealed).map(Some),
    }
}

/// Saves a tool that answered, its key sealed.
pub fn add(conn: &Connection, name: &str, link: &str, key: Option<&str>) -> Result<Tool, Error> {
    let (id, ts) = (ulid(now_us()), now_us());
    let sealed = key.map(|k| seal::seal(&id, k)).transpose()?;
    conn.execute(
        "INSERT INTO tools (id, name, link, enabled, key, added_ts) VALUES (?1, ?2, ?3, 1, ?4, ?5)",
        params![id, name, link, sealed, ts],
    )?;
    get(conn, &id)
}

pub fn set_key(conn: &Connection, id: &str, key: &str) -> Result<(), Error> {
    get(conn, id)?;
    let sealed = seal::seal(id, key)?;
    conn.execute(
        "UPDATE tools SET key = ?2 WHERE id = ?1",
        params![id, sealed],
    )?;
    Ok(())
}

pub fn set_enabled(conn: &Connection, id: &str, on: bool) -> Result<(), Error> {
    get(conn, id)?;
    conn.execute(
        "UPDATE tools SET enabled = ?2 WHERE id = ?1",
        params![id, on],
    )?;
    Ok(())
}

/// Removes the tool and its key.
pub fn remove(conn: &Connection, id: &str) -> Result<(), Error> {
    get(conn, id)?;
    conn.execute("DELETE FROM tools WHERE id = ?1", [id])?;
    seal::forget(id);
    Ok(())
}

/// Each enabled tool's entry name for an engine, with its id: a slug of its name, unique and never
/// `gathr`, so its actions read as `mcp__<slug>__<action>`.
pub fn entries(conn: &Connection) -> Result<Vec<(String, String)>, Error> {
    let mut taken = vec!["gathr".to_string()];
    let mut entries = Vec::new();
    for tool in list(conn)?.into_iter().filter(|t| t.enabled) {
        let base = slug(&tool.name);
        let mut name = base.clone();
        let mut n = 1;
        while taken.contains(&name) {
            n += 1;
            name = format!("{base}_{n}");
        }
        taken.push(name.clone());
        entries.push((name, tool.id));
    }
    Ok(entries)
}

/// Lowercase letters and digits, each other run as one `_`.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        "tool".into()
    } else {
        slug.into()
    }
}

/// The link's host and port, refused unless it's https, or http to this computer.
pub fn checked(link: &str) -> Result<String, Error> {
    let refused = || Error::Refused("Use an https link, or http on this computer.".into());
    let (scheme, rest) = link.trim().split_once("://").ok_or_else(refused)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err(refused());
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => format!("[{}]", v6.split(']').next().unwrap_or("")),
        None => authority.split(':').next().unwrap_or("").to_string(),
    };
    let local = matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]");
    match scheme.to_ascii_lowercase().as_str() {
        "https" => Ok(authority.to_string()),
        "http" if local => Ok(authority.to_string()),
        _ => Err(refused()),
    }
}

/// A tool connected: rmcp's client, which closes when dropped.
pub type Remote = RunningService<RoleClient, ()>;

/// Connects to the tool at `link` with `key`, waiting `wait` at most for its answer.
pub async fn connect(link: &str, key: Option<&str>, wait: Duration) -> Result<Remote, Problem> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(link.trim());
    if let Some(key) = key {
        config = config.auth_header(key);
    }
    let transport = StreamableHttpClientTransport::from_config(config);
    let failed = match tokio::time::timeout(wait, ().serve(transport)).await {
        Ok(Ok(remote)) => return Ok(remote),
        Ok(Err(e)) => e,
        Err(_) => return Err(Problem::NotReachable),
    };
    // rmcp keeps the transport's error out of the chain's sources: it's read from there.
    let (asked, challenge) = match &failed {
        ClientInitializeError::TransportError { error, .. } => refusal(error),
        other => refusal(other),
    };
    if !asked {
        return Err(Problem::NotReachable);
    }
    if key.is_some() {
        return Err(Problem::KeyRefused);
    }
    // MCP's authorization spec (2025-11-25): a server that signs in through the browser names its
    // protected resource metadata in WWW-Authenticate, or serves it at the well-known address.
    if challenge.is_some_and(|c| c.contains("resource_metadata")) || signs_in(link).await {
        return Err(Problem::SignIn);
    }
    Err(Problem::NeedsKey)
}

/// Whether the error is the tool refusing access (a 401 or 403), and its WWW-Authenticate if any.
fn refusal(e: &(dyn std::error::Error + 'static)) -> (bool, Option<String>) {
    let (mut asked, mut challenge) = (false, None);
    let mut at = Some(e);
    while let Some(error) = at {
        if let Some(auth) = error.downcast_ref::<AuthRequiredError>() {
            asked = true;
            challenge = Some(auth.www_authenticate_header.clone());
        }
        let text = error.to_string();
        asked |= error.downcast_ref::<InsufficientScopeError>().is_some()
            || text.contains("HTTP 401")
            || text.contains("HTTP 403")
            || text.contains("Auth required");
        at = error.source();
    }
    (asked, challenge)
}

/// Whether the tool serves OAuth protected resource metadata (RFC 9728) at the well-known address,
/// for its link's path or at its root. Asked without the key.
async fn signs_in(link: &str) -> bool {
    static NEVER: AtomicBool = AtomicBool::new(false);
    let Some((origin, path)) = link
        .split_once("://")
        .map(|(scheme, rest)| match rest.find('/') {
            Some(at) => (
                format!("{scheme}://{}", &rest[..at]),
                rest[at..].to_string(),
            ),
            None => (format!("{scheme}://{rest}"), String::new()),
        })
    else {
        return false;
    };
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/');
    let mut urls = vec![format!(
        "{origin}/.well-known/oauth-protected-resource{path}"
    )];
    if !path.is_empty() {
        urls.push(format!("{origin}/.well-known/oauth-protected-resource"));
    }
    tokio::task::spawn_blocking(move || {
        urls.iter().any(|url| {
            fetch::fetch(url, fetch::Redirects::Refused, &NEVER)
                .is_ok_and(|text| text.contains("authorization_servers"))
        })
    })
    .await
    .unwrap_or(false)
}

/// The name the tool gives itself: its title, else its name.
pub fn own_name(remote: &Remote) -> Option<String> {
    let info = remote.peer_info()?.server_info.clone()?;
    let name = info.title.clone().unwrap_or_else(|| info.name.clone());
    Some(name.trim().to_string()).filter(|n| !n.is_empty())
}

/// Everything the tool lets an agent do, each with its description's first line.
pub async fn actions(remote: &Remote, wait: Duration) -> Result<Vec<Action>, Problem> {
    let listed = tokio::time::timeout(wait, remote.list_all_tools()).await;
    let tools = listed
        .map_err(|_| Problem::NotReachable)?
        .map_err(|_| Problem::NotReachable)?;
    Ok(tools
        .into_iter()
        .map(|t| Action {
            name: t.name.to_string(),
            description: t.description.and_then(|d| {
                d.lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .map(String::from)
            }),
        })
        .collect())
}

/// One page of the tool's actions as MCP lists them, for the relay.
pub async fn list_page(
    remote: &Remote,
    page: Option<PaginatedRequestParams>,
) -> Result<ListToolsResult, ErrorData> {
    let listed = tokio::time::timeout(CONNECT, remote.list_tools(page)).await;
    listed.map_err(|_| unanswered())?.map_err(relayed)
}

/// One call of an action, for the relay.
pub async fn call(
    remote: &Remote,
    request: CallToolRequestParams,
) -> Result<CallToolResponse, ErrorData> {
    let called = tokio::time::timeout(CALL, remote.call_tool_once(request)).await;
    called.map_err(|_| unanswered())?.map_err(relayed)
}

fn unanswered() -> ErrorData {
    ErrorData::internal_error("The tool didn't answer in time", None)
}

/// The tool's own error as it gave it; anything else in plain words.
fn relayed(e: ServiceError) -> ErrorData {
    match e {
        ServiceError::McpError(e) => e,
        _ => ErrorData::internal_error("The tool can't be reached", None),
    }
}
