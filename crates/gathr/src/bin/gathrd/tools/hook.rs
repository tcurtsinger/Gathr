//! Gathr's hook (0029): Claude Code asks it before each file write and command, with the engine's
//! own token. It answers "ask" before a push or a write outside the agent's folders (0034), which
//! sends the call to the person as an approval whatever the agent's mode or a project's rules allow;
//! a write outside that an earlier Allow in the chat covers is answered "allow". With no answer the
//! engine's own mode would ask again for a file outside its folders (acceptEdits does), and the
//! card said the Allow covers the folder.

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

/// A `git push` from the engine's worktree, however it was run: `allow` at once when it isn't
/// risky (`gathr::pushes`), else once the person allows it (or the agent's mode is Full, or they
/// just allowed the command that runs it); `deny` otherwise. It waits as long as the person takes;
/// a push whose hook gives up is ended by git.
async fn pre_push(
    Extract(s): Extract<Arc<State>>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    refs: String,
) -> String {
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
    Json(input): Json<Value>,
) -> Json<Value> {
    use gathr::approvals::{GATHR_ASKS, OUTSIDE};
    let tool = input["tool_name"].as_str().unwrap_or("");
    let tool_input = &input["tool_input"];
    let asked = gathr::approvals::ask(
        tool,
        tool_input,
        &caller.folders,
        s.chats.data.path(),
        caller.from_before,
    );
    // Empty: the agent's mode decides.
    let Some(why) = asked else {
        return Json(json!({}));
    };
    let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
    let event = |name: &str| {
        let event = Event::point(name).trace(&trace).chat(&caller.chat).ok();
        let event = event.attr("gen_ai.tool.name", tool.to_string());
        event.attr("gen_ai.agent.id", caller.agent.clone())
    };
    let answer = |decision: &str, reason: String| {
        Json(
            json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse",
            "permissionDecision": decision, "permissionDecisionReason": reason } }),
        )
    };
    // A write outside the agent's folders that an earlier Allow in this chat covers.
    let path = gathr::approvals::written(tool, tool_input, &caller.folders);
    let path = path.filter(|_| why == OUTSIDE);
    let agent = caller.agent.as_deref();
    let covering = (path.as_ref()).and_then(|p| s.chats.covering(agent, &caller.chat, p));
    if let Some(folder) = covering {
        let folder = folder.display().to_string();
        let reason = format!("The person allowed writes in {folder} in this chat.");
        s.writer
            .record(event("gathr.approvals.covered").attr("folder", folder));
        return answer("allow", reason);
    }
    s.writer
        .record(event("gathr.approvals.ask").attr("why", why));
    let reason = match path.and_then(|p| gathr::approvals::cover(&p)) {
        Some(cover) => format!("{GATHR_ASKS} {why}. {}", cover.said()),
        None => format!("{GATHR_ASKS} {why}"),
    };
    answer("ask", reason)
}
