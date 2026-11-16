//! Gathr's hook (0029): Claude Code asks it before each file write and command, with the engine's
//! own token. It answers "ask" before a push or a write outside the agent's folders (0034), which
//! sends the call to the person as an approval whatever the agent's mode or a project's rules allow.
//! A `wait` isn't on Claude Code's allow list, so the hook answers each one: "allow" with no check
//! or a read-only one, "ask" for any other check, and nothing in auto mode, where Claude Code's own
//! classifier reads the check as it reads the agent's commands (owner, 2026-11-11).
//! A held turn's writes into its worktree and its pushes are refused, in every mode (0063).

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Extension, RawQuery, State as Extract};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use gathr::claude::command::{AUTO_QUERY, FULL_QUERY, WAIT_TOOL};
use gathr::event::Event;
use gathr::hold;
use rusqlite::Connection;
use serde_json::{Value, json};

use super::Caller;
use crate::api::State;

/// Where the hook is: admitted with a running engine's token only, like `/mcp`.
pub const HOOK_PATH: &str = "/hook";
/// Where an agent's worktree's `pre-push` hook asks, with its engine's token too.
pub const PUSH_PATH: &str = "/hook/push";
/// How long the hooks wait for the hold: Claude Code's hook blocks the call when it gets no answer.
const HOLD_READ: Duration = Duration::from_secs(5);

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(HOOK_PATH, post(pre_tool_use))
        .route(PUSH_PATH, post(pre_push))
}

/// A `git push` from the engine's worktree, however it was run: refused while its turn is held
/// (0063); else `allow` at once when it isn't risky (`gathr::pushes`), else once the person allows
/// it (or the agent's mode is Full, or they just allowed the command that runs it); `deny`
/// otherwise. It waits as long as the person takes; a push whose hook gives up is ended by git.
async fn pre_push(
    Extract(s): Extract<Arc<State>>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    refs: String,
) -> String {
    if let Some(why) = held(&s, &caller, None, |c, turn| hold::push_held(c, turn)).await {
        return why;
    }
    let header = |name: &str| {
        let value = headers.get(name).and_then(|v| v.to_str().ok());
        value.unwrap_or_default().to_string()
    };
    let (remote, url) = (header("gathr-remote"), header("gathr-url"));
    let refs: Vec<&str> = refs.lines().filter(|l| !l.trim().is_empty()).collect();
    let env = tokio::task::spawn_blocking(|| {
        gathr::platform::env::fresh(std::env::vars_os().collect()).vars
    });
    let risk = match env.await {
        Ok(env) => gathr::pushes::why_ask(&caller.cwd, &remote, &url, &refs, &env).await,
        Err(_) => Some("Gathr couldn't read the person's environment".into()),
    };
    let input = json!({ "command": format!("git push {remote}"), "remote": remote, "url": url,
        "refs": refs, "risk": risk });
    // A service that stops refuses the push rather than wait: its server waits on this request.
    let allowed = tokio::select! {
        allowed = s.chats.ask_push(&caller, input) => allowed,
        () = s.stopped() => false,
    };
    match allowed {
        true => "allow",
        false => "deny",
    }
    .to_string()
}

async fn pre_tool_use(
    Extract(s): Extract<Arc<State>>,
    Extension(caller): Extension<Caller>,
    RawQuery(query): RawQuery,
    Json(input): Json<Value>,
) -> Json<Value> {
    let tool = input["tool_name"].as_str().unwrap_or("");
    let decide = |decision: &str, reason: String| {
        Json(
            json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse",
        "permissionDecision": decision, "permissionDecisionReason": reason } }),
        )
    };
    if query.as_deref() == Some(FULL_QUERY) {
        // Full never asks: its hook answers only the hold.
        return match held_write(&s, &caller, tool, &input["tool_input"]).await {
            Some(why) => decide("deny", why),
            None => Json(json!({})),
        };
    }
    let Some(why) = gathr::approvals::ask(
        tool,
        &input["tool_input"],
        &caller.folders,
        s.chats.data.path(),
        caller.from_before,
    ) else {
        if let Some(why) = held_write(&s, &caller, tool, &input["tool_input"]).await {
            return decide("deny", why);
        }
        return match tool == WAIT_TOOL {
            true => decide(
                "allow",
                "Gathr allows a wait with no check to ask about".into(),
            ),
            // Empty: the agent's mode decides.
            false => Json(json!({})),
        };
    };
    if tool == WAIT_TOOL && query.as_deref() == Some(AUTO_QUERY) {
        // Empty: auto mode's classifier decides, and `gathr.approvals.check` says so once the
        // call arrives.
        return Json(json!({}));
    }
    let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
    let event = Event::point("gathr.approvals.ask")
        .trace(&trace)
        .chat(&caller.chat)
        .ok()
        .attr("gen_ai.tool.name", tool.to_string())
        .attr("gen_ai.agent.id", caller.agent.clone())
        .attr("why", why);
    s.writer.record(event);
    decide("ask", format!("{} {why}", gathr::approvals::GATHR_ASKS))
}

/// A held turn's file write into its worktree: the deny reason. Only a write there reads the hold.
async fn held_write(s: &State, caller: &Caller, tool: &str, input: &Value) -> Option<String> {
    if !gathr::approvals::writes_in(tool, input, &caller.cwd) {
        return None;
    }
    let writer = caller.clone();
    let read = move |c: &mut Connection, turn: &str| hold::write_held(c, turn, &writer.place());
    held(s, caller, Some(tool), read).await
}

/// The hold for the caller's turn, read in one writer job of at most `HOLD_READ`: the answer when
/// held, with its refusal recorded in a second job, not waited on. Not held when it can't be read.
async fn held(
    s: &State,
    caller: &Caller,
    tool: Option<&str>,
    read: impl FnOnce(&mut Connection, &str) -> Result<Found, gathr::Error> + Send + 'static,
) -> Option<String> {
    let on = caller.on()?;
    let turn = on.turn.clone();
    let job = s.writer.call(move |c| read(c, &turn));
    let read = match tokio::time::timeout(HOLD_READ, job).await {
        Err(_) => Read::Late,
        Ok(None) => Read::Stopped,
        Ok(Some(found)) => Read::Done(Box::new(found)),
    };
    let (found, failed) = answer(read, &on.trace, &caller.chat);
    if let Some(failed) = failed {
        s.writer
            .record(failed.attr("gen_ai.agent.id", caller.agent.clone()));
    }
    let (why, mut refusal) = found?;
    refusal.tool = tool.map(String::from);
    let writer = s.writer.clone();
    tokio::spawn(async move { writer.call(move |c| hold::refused(c, &refusal)).await });
    Some(why)
}

/// What a hold read found: the deny or push answer, and the refusal to record.
type Found = Option<(String, hold::Refusal)>;

/// How a bounded hold read ended.
enum Read {
    Late,
    Stopped,
    Done(Box<Result<Found, gathr::Error>>),
}

/// A hold read's outcome as the hooks take it: what it found, or not held, with
/// `gathr.hold.read` saying why, so a hold Gathr can't read never blocks the write or push.
fn answer(read: Read, trace: &str, chat: &str) -> (Found, Option<Event>) {
    let (error, why) = match read {
        Read::Done(done) => match *done {
            Ok(found) => return (found, None),
            Err(e) => ("sqlite", e.to_string()),
        },
        Read::Late => (
            "timeout",
            format!("the hold wasn't read in {} s", HOLD_READ.as_secs()),
        ),
        Read::Stopped => ("writer", "the writer has stopped".to_string()),
    };
    let event = Event::point("gathr.hold.read")
        .trace(trace)
        .chat(chat)
        .error(error)
        .attr("why", why);
    (None, Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hook in Ask, Edits and Auto blocks the call when it fails, so a hold read that fails or
    /// comes late reads as not held, and says why (0063 I2.2).
    #[test]
    fn a_failed_hold_read_lets_the_write_go_on() {
        let failed = [
            (Read::Late, "timeout"),
            (Read::Stopped, "writer"),
            (
                Read::Done(Box::new(Err(gathr::Error::Other("disk I/O error".into())))),
                "sqlite",
            ),
        ];
        for (read, error) in failed {
            let (held, event) = answer(read, "abcdef0123456789", "c");
            assert!(held.is_none(), "{error}: held");
            let event = event.unwrap_or_else(|| panic!("{error}: nothing recorded"));
            assert_eq!(
                (event.name.as_str(), event.error_type.as_deref()),
                ("gathr.hold.read", Some(error))
            );
            assert!(event.attrs["why"].is_string(), "{error}: {:?}", event.attrs);
        }
        let (held, event) = answer(Read::Done(Box::new(Ok(None))), "abcdef0123456789", "c");
        assert!(
            held.is_none() && event.is_none(),
            "a read that worked recorded a failure"
        );
    }
}
