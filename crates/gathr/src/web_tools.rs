//! Tools that live on the web: remote MCP servers, by their link and, when one needs it, a key or a
//! sign-in through the browser (`tool_sign_in`). Added
//! once for every agent; gathrd relays to each, so an engine never sees a tool's link's key. Gathr
//! calls out with rmcp's own client: only https, or http to this computer; a redirect is followed
//! only to the link's own origin, so the key never reaches another host; a response is capped by
//! rmcp at 16 MB.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListToolsResult, PaginatedRequestParams,
};
use rmcp::service::{ClientInitializeError, RoleClient, RunningService, ServiceError};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::{AuthClient, AuthError, AuthorizationManager};
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, InsufficientScopeError, StreamableHttpClientTransportConfig,
};
use rmcp::{ErrorData, ServiceExt};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::ids::{now_us, ulid};
use crate::tool_sign_in::{self, SignIn};
use crate::writer::Writer;
use crate::{Error, fetch, seal};

/// How long a tool has to answer Gathr's hello: its connect timeout, in effect.
pub const CONNECT: Duration = Duration::from_secs(10);
/// How long a list of tools waits on each one's state, so a dead tool never holds the list.
pub const STATE: Duration = Duration::from_secs(5);
/// What an agent is told when a tool doesn't answer as one.
pub const UNREACHABLE: &str = "The tool can't be reached";
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
    /// Whether it signed in through the browser; never the sign-in.
    pub signed_in: bool,
    /// The Client ID it signed in with, where its sign-in service takes no app it doesn't know: not a
    /// secret. Never its Client secret.
    pub client_id: Option<String>,
    pub added_ts: i64,
    /// `web`, or `pc` for a program Gathr runs on this PC (`pc_tools`), whose link is empty.
    pub kind: &'static str,
    /// The AI app outside Gathr that added it, if one did.
    pub added_by: Option<String>,
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
    /// It signs in through the browser (OAuth), and isn't signed in: not yet, or no longer.
    SignIn,
    /// Nothing that speaks MCP answered.
    NotReachable,
}

const COLUMNS: &str = "id, name, link, enabled, key IS NOT NULL, sign_in IS NOT NULL, client_id, added_ts, program IS NOT NULL,
     (SELECT app FROM added_by WHERE kind = 'tool' AND added_by.id = tools.id)";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Tool> {
    Ok(Tool {
        id: r.get(0)?,
        name: r.get(1)?,
        link: r.get(2)?,
        enabled: r.get(3)?,
        has_key: r.get(4)?,
        signed_in: r.get(5)?,
        client_id: r.get(6)?,
        added_ts: r.get(7)?,
        kind: if r.get(8)? { "pc" } else { "web" },
        added_by: r.get(9)?,
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

/// What Gathr connects to a tool with.
pub enum Access {
    /// Nothing: it needs nothing.
    Open,
    Key(String),
    SignIn(Arc<SignIn>),
}

/// The tool's sign-in, or its key unsealed, or nothing. `writer` is where a renewed sign-in is saved.
pub fn access(conn: &Connection, id: &str, writer: &Writer) -> Result<Access, Error> {
    type Sealed = Option<Vec<u8>>;
    let sealed: Option<(Sealed, Sealed)> = conn
        .query_row("SELECT key, sign_in FROM tools WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    match sealed {
        None => Err(Error::Refused(format!("There's no tool {id}"))),
        Some((_, Some(sign_in))) => tool_sign_in::kept(id, writer, &sign_in).map(Access::SignIn),
        Some((Some(key), None)) => seal::unseal(id, &key).map(Access::Key),
        Some((None, None)) => Ok(Access::Open),
    }
}

/// Saves a tool that answered, its key sealed.
pub fn add(conn: &Connection, name: &str, link: &str, key: Option<&str>) -> Result<Tool, Error> {
    let id = ulid(now_us());
    let sealed = key.map(|k| seal::seal(&id, k)).transpose()?;
    insert(conn, &id, name, link, sealed, None, None)
}

/// Saves a tool that signed in, as `id`, with its sign-in sealed by `tool_sign_in::sealed` and the
/// Client ID it signed in with, if any.
pub fn add_signed_in(
    conn: &Connection,
    id: &str,
    name: &str,
    link: &str,
    sealed: &[u8],
    client_id: Option<&str>,
) -> Result<Tool, Error> {
    insert(conn, id, name, link, None, Some(sealed), client_id)
}

fn insert(
    conn: &Connection,
    id: &str,
    name: &str,
    link: &str,
    key: Option<Vec<u8>>,
    sign_in: Option<&[u8]>,
    client_id: Option<&str>,
) -> Result<Tool, Error> {
    conn.execute(
        "INSERT INTO tools (id, name, link, enabled, key, sign_in, client_id, added_ts)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
        params![id, name, link, key, sign_in, client_id, now_us()],
    )?;
    get(conn, id)
}

/// Keeps a new or renewed sign-in, sealed by `tool_sign_in::sealed`.
pub fn set_sign_in(conn: &Connection, id: &str, sealed: &[u8]) -> Result<(), Error> {
    get(conn, id)?;
    conn.execute(
        "UPDATE tools SET sign_in = ?2 WHERE id = ?1",
        params![id, sealed],
    )?;
    Ok(())
}

/// Keeps the Client ID a new sign-in was made with, or none.
pub fn set_client_id(conn: &Connection, id: &str, client_id: Option<&str>) -> Result<(), Error> {
    get(conn, id)?;
    conn.execute(
        "UPDATE tools SET client_id = ?2 WHERE id = ?1",
        params![id, client_id],
    )?;
    Ok(())
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

/// Removes the tool, its key and its sign-in, with its Client ID and secret, or its variables.
pub fn remove(conn: &Connection, id: &str) -> Result<(), Error> {
    get(conn, id)?;
    conn.execute("DELETE FROM tools WHERE id = ?1", [id])?;
    crate::added_by::forget(conn, "tool", id)?;
    seal::forget(id);
    seal::forget(&crate::pc_tools::sealed_as(id));
    seal::forget(&tool_sign_in::sealed_as(id));
    tool_sign_in::forget(id);
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

/// What an engine is given of the tools as it starts, as one text: each enabled tool's entry and id,
/// and what a tool on this PC runs. An engine started with another text starts again to see the change.
pub fn given(conn: &Connection) -> Result<String, Error> {
    let mut given = String::new();
    for (name, id) in entries(conn)? {
        let runs: (Option<String>, Option<String>, Option<String>) = conn.query_row(
            "SELECT program, args, folder FROM tools WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        given.push_str(&format!("{name}\t{id}\t{runs:?}\n"));
    }
    Ok(given)
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
    let refused = || Error::Refused("Use https, or http on this computer.".into());
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

/// Connects to the tool at `link` with `access`, waiting `wait` at most for its answer. A sign-in is
/// renewed by rmcp's client on its own when it runs out, at any call.
pub async fn connect(link: &str, access: &Access, wait: Duration) -> Result<Remote, Problem> {
    let config = StreamableHttpClientTransportConfig::with_uri(link.trim());
    let hello = async {
        match access {
            Access::Open => Ok(
                ().serve(StreamableHttpClientTransport::from_config(config))
                    .await,
            ),
            Access::Key(key) => {
                let transport = StreamableHttpClientTransport::from_config(config.auth_header(key));
                Ok(().serve(transport).await)
            }
            Access::SignIn(sign_in) => match tool_sign_in::manager(link, sign_in).await {
                Ok(Some(manager)) => Ok(signed_in(config, manager).await.map_err(|e| *e)),
                Ok(None) => Err(Problem::SignIn),
                Err(_) => Err(Problem::NotReachable),
            },
        }
    };
    let served = match tokio::time::timeout(wait, hello).await {
        Ok(served) => served?,
        Err(_) => return Err(Problem::NotReachable),
    };
    problem(link, matches!(access, Access::Key(_)), served).await
}

/// Connects to the tool at `link` with the manager a sign-in just finished in.
pub async fn connect_signed_in(
    link: &str,
    manager: AuthorizationManager,
    wait: Duration,
) -> Result<Remote, Problem> {
    let config = StreamableHttpClientTransportConfig::with_uri(link.trim());
    match tokio::time::timeout(wait, signed_in(config, manager)).await {
        Ok(served) => problem(link, false, served.map_err(|e| *e)).await,
        Err(_) => Err(Problem::NotReachable),
    }
}

async fn signed_in(
    config: StreamableHttpClientTransportConfig,
    manager: AuthorizationManager,
) -> Result<Remote, Box<ClientInitializeError>> {
    let client = AuthClient::new(http_client(AuthorizationManager::with_client), manager);
    ().serve(StreamableHttpClientTransport::with_client(client, config))
        .await
        .map_err(Box::new)
}

/// A default HTTP client of the kind rmcp's transport takes, named through an rmcp function that
/// takes one: Gathr has no HTTP library of its own. It drops the Authorization header on a redirect
/// to another host.
fn http_client<C: Default>(_: fn(&mut AuthorizationManager, C) -> Result<(), AuthError>) -> C {
    C::default()
}

/// What a tool's answer to Gathr's hello means.
async fn problem(
    link: &str,
    keyed: bool,
    served: Result<Remote, ClientInitializeError>,
) -> Result<Remote, Problem> {
    let failed = match served {
        Ok(remote) => return Ok(remote),
        Err(e) => e,
    };
    // rmcp keeps the transport's error out of the chain's sources: it's read from there.
    let (asked, challenge) = match &failed {
        ClientInitializeError::TransportError { error, .. } => refusal(error),
        other => refusal(other),
    };
    if !asked {
        return Err(Problem::NotReachable);
    }
    if keyed {
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
        _ => ErrorData::internal_error(UNREACHABLE, None),
    }
}
