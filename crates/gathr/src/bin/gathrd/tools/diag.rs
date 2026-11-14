//! `diag` (0019): Gathr's own record of what went wrong, read-only, for any agent. The command
//! line's own answers, of the caller's chat and of no chat, with secrets hidden and a cap.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::data::DataDir;
use gathr::{Error, db, diag};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::Value;

use super::server::{Tools, caller_of};

/// About 4,000 tokens: an answer past it is cut, and says so.
const CAP: usize = 16_000;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Diag {
    /// `health`, `failures` or `trace`.
    command: String,
    /// For `failures`: how far back, like 30m, 24h or 7d. 24h unless set.
    since: Option<String>,
    /// For `trace`: the reference, as in "(ref 1a2b3c4d)": 8 or more hex characters.
    reference: Option<String>,
}

#[tool_router(router = diag_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Gathr's own record of what went wrong, read-only. health: the service, accounts, this chat's open \
                       tasks and waits, the crash log. failures: errors since a time (since, default 24h), grouped by \
                       cause, with references. trace: everything behind a reference, as in '(ref 1a2b3c4d)', errors \
                       first. Only this chat's events and events of no chat. Secrets are hidden. A long answer is cut \
                       and says so."
    )]
    async fn diag(
        &self,
        Parameters(ask): Parameters<Diag>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let answer = answer(self.chats.data.clone(), caller.chat.clone(), ask).await;
        Ok(self.respond(&caller, "diag", started, answer).await)
    }
}

/// Read on a thread of its own, as the command line reads: not on the writer, which a long scan
/// would hold, nor on the async workers, as `health` asks this service over HTTP.
async fn answer(data: DataDir, chat: String, ask: Diag) -> Result<String, String> {
    let since = match ask.command.as_str() {
        "failures" => Some(
            diag::parse_since(ask.since.as_deref().unwrap_or("24h")).map_err(|e| e.to_string())?,
        ),
        _ => None,
    };
    let read = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let chat = Some(chat.as_str());
        match (ask.command.as_str(), since) {
            ("health", _) => Ok(diag::health(&data, chat)),
            ("failures", Some(since)) => {
                let (conn, _) = db::open_read_only(&data.db())?;
                diag::failures(&conn, gathr::ids::now_us() - since, chat)
            }
            ("trace", _) => {
                let (conn, _) = db::open_read_only(&data.db())?;
                diag::trace(&conn, ask.reference.as_deref().unwrap_or_default(), chat)
            }
            (other, _) => Err(Error::Other(format!(
                "{other:?} isn't a command: use health, failures or trace"
            ))),
        }
    });
    let read = read.await.map_err(|e| e.to_string())?;
    let mut answer = gathr::redact::redact_json(&read.map_err(|e| e.to_string())?);
    // Errors first, each part still in time order: a cut never loses the failure asked about.
    if let Some(events) = answer.get_mut("events").and_then(Value::as_array_mut) {
        events.sort_by_key(|e| e["status"] != "error");
    }
    // Compact: an agent pays for every character, and indentation was two in five of them.
    let text = serde_json::to_string(&answer).map_err(|e| e.to_string())?;
    Ok(capped(text))
}

/// `text` within `CAP` characters, with a last line that says how much was cut.
fn capped(text: String) -> String {
    let total = text.chars().count();
    if total <= CAP {
        return text;
    }
    let cut = gathr::text::cut(&text, CAP, true);
    let kept = cut.trim_end_matches('…');
    let left = total - kept.chars().count();
    format!(
        "{kept}\n[… {left} of {total} characters cut: the answer above is not whole JSON; ask failures with a \
         shorter since, or trace one reference …]"
    )
}
