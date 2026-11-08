//! Gathr's hook (0029): Claude Code asks it before each file write and command, with the engine's
//! own token. It answers "ask" before a push or a write outside the engine's folder, which sends
//! the call to the person as an approval whatever the agent's mode or a project's rules allow.

use std::sync::Arc;

use axum::extract::{Extension, State as Extract};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use gathr::event::Event;
use serde_json::{Value, json};

use super::Caller;
use crate::api::State;

/// Where the hook is: admitted with a running engine's token only, like `/mcp`.
pub const HOOK_PATH: &str = "/hook";
/// Where an agent's worktree's `pre-push` hook asks, with its engine's token too.
pub const PUSH_PATH: &str = "/hook/push";

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(HOOK_PATH, post(pre_tool_use))
        .route(PUSH_PATH, post(pre_push))
}

/// A `git push` from the engine's worktree, however it was run: `allow` once the person allows
/// it (or the agent's mode is Full, or they just allowed the command that runs it), else `deny`.
/// It waits as long as the person takes; a push whose hook gives up is ended by git.
async fn pre_push(
    Extract(s): Extract<Arc<State>>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    refs: String,
) -> &'static str {
    let header = |name: &str| {
        let value = headers.get(name).and_then(|v| v.to_str().ok());
        value.unwrap_or_default().to_string()
    };
    let (remote, url) = (header("gathr-remote"), header("gathr-url"));
    let refs: Vec<&str> = refs.lines().filter(|l| !l.trim().is_empty()).collect();
    let input = json!({ "command": format!("git push {remote}"), "remote": remote, "url": url,
        "refs": refs });
    // A service that stops refuses the push rather than wait: its server waits on this request.
    let allowed = tokio::select! {
        allowed = s.chats.ask_push(&caller, input) => allowed,
        () = s.stopped() => false,
    };
    match allowed {
        true => "allow",
        false => "deny",
    }
}

async fn pre_tool_use(
    Extract(s): Extract<Arc<State>>,
    Extension(caller): Extension<Caller>,
    Json(input): Json<Value>,
) -> Json<Value> {
    let tool = input["tool_name"].as_str().unwrap_or("");
    let Some(why) = gathr::approvals::ask(tool, &input["tool_input"], &caller.cwd) else {
        // Empty: the agent's mode decides.
        return Json(json!({}));
    };
    let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
    let event = Event::point("gathr.approvals.ask")
        .trace(&trace)
        .chat(&caller.chat)
        .ok()
        .attr("gen_ai.tool.name", tool.to_string())
        .attr("gen_ai.agent.id", caller.agent.clone())
        .attr("why", why);
    s.writer.record(event);
    let reason = format!("Gathr asks because {why}");
    Json(
        json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse",
        "permissionDecision": "ask", "permissionDecisionReason": reason } }),
    )
}
