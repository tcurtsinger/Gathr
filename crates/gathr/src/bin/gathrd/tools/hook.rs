//! Gathr's hook (0029): Claude Code asks it before each file write and command, with the engine's
//! own token. It answers "ask" before a push or a write outside the engine's folder, which sends
//! the call to the person as an approval whatever the agent's mode or a project's rules allow.

use std::sync::Arc;

use axum::extract::{Extension, State as Extract};
use axum::routing::post;
use axum::{Json, Router};
use gathr::event::Event;
use serde_json::{Value, json};

use super::Caller;
use crate::api::State;

/// Where the hook is: admitted with a running engine's token only, like `/mcp`.
pub const HOOK_PATH: &str = "/hook";

pub fn routes() -> Router<Arc<State>> {
    Router::new().route(HOOK_PATH, post(pre_tool_use))
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
