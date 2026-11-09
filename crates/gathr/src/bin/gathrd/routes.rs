//! The chat and engine routes, and the live WebSocket (docs/first-slice.md).

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::claude::command;
use gathr::event::Event;
use gathr::store;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::api::{State, Trace, failure};
use crate::chats::Cmd;

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats", get(list).post(crate::team::create))
        .route("/api/chats/{chat}", get(history).patch(crate::team::change))
        .route("/api/chats/{chat}/messages", post(send))
        .route("/api/chats/{chat}/stop", post(stop))
        .route("/api/chats/{chat}/approvals/{request}", post(approve))
        .route("/api/chats/{chat}/turns", get(turns))
        .route("/api/trace/{reference}", get(trace))
        .route("/api/ui/events", post(ui_event))
        .route("/api/engines/claude", get(engine))
        .route("/api/engines/claude/update", post(update_engine))
        .route("/api/live", get(live))
}

pub type Answer = Result<Json<Value>, Fail>;

/// A failed answer, kept small; it becomes the API's error JSON with its reference.
pub struct Fail(StatusCode, &'static str, String, String);

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        failure(self.0, self.1, Some(&self.2), &self.3)
    }
}

pub fn fail(status: StatusCode, error: &'static str, why: &str, trace: &str) -> Fail {
    Fail(status, error, why.to_string(), trace.to_string())
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    db(&s, &t, "Chats failed", |conn| store::chats(conn)).await
}

async fn history(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (approvals, streaming) = s.chats.open(&chat).await;
    let id = chat.clone();
    let Json(mut history) =
        db(&s, &t, "Chat failed", move |conn| store::history(conn, &id)).await?;
    history["approvals"] = json!(approvals);
    history["streaming"] = json!(still_streaming(&history, streaming));
    crate::team::with_folders(&s, &chat, &mut history);
    Ok(Json(history))
}

/// The replies still streaming whose text isn't in `history` yet: a block saved between the two
/// reads shows once, as its message.
fn still_streaming(history: &Value, streaming: Vec<Value>) -> Vec<Value> {
    let messages = history["messages"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let replies = |turn: &Value| {
        let of_turn = |m: &&Value| m["turn"] == *turn && m["role"] == "assistant";
        messages.iter().filter(of_turn).count()
    };
    let unsaved = streaming
        .into_iter()
        .filter(|s| s["saved"].as_i64() == Some(replies(&s["turn"]) as i64));
    unsaved
        .map(|mut s| {
            s.as_object_mut().map(|o| o.remove("saved"));
            s
        })
        .collect()
}

#[derive(Deserialize)]
struct NewMessage {
    text: String,
    agent: Option<String>,
}

async fn send(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Json(body): Json<NewMessage>,
) -> Answer {
    known(&s, &t, &chat).await?;
    if body.text.trim().is_empty() {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "Send failed",
            "the message is empty",
            &t.0,
        ));
    }
    if let Some(agent) = &body.agent {
        crate::agents::find(&s, &t, agent, "Send failed").await?;
    }
    let agent = addressee(&s, &t, &chat, body.agent, &body.text).await?;
    let (reply, answer) = oneshot::channel();
    let task = s.chats.task(&chat, agent.as_deref());
    let command = Cmd::Send {
        chat,
        text: body.text,
        agent,
        trace: t.0.clone(),
        reply,
    };
    let _ = task.send(command);
    match answer.await {
        Ok(Ok(sent)) => Ok(Json(sent)),
        Ok(Err(failed)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Send failed",
            &failed.why,
            &failed.trace,
        )),
        Err(_) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Send failed",
            "the chat's task ended",
            &t.0,
        )),
    }
}

async fn stop(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    Ok(Json(json!({ "stopped": s.chats.stop(&chat).await })))
}

#[derive(Deserialize)]
struct Approval {
    allow: bool,
}

async fn approve(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, request)): Path<(String, String)>,
    Json(body): Json<Approval>,
) -> Answer {
    match s.chats.approve(&chat, &request, body.allow).await {
        true => Ok(Json(json!({ "answered": true }))),
        false => Err(fail(
            StatusCode::NOT_FOUND,
            "Answer failed",
            "no such question is open",
            &t.0,
        )),
    }
}

/// The chat's turns with their events, for Diagnostics.
async fn turns(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    db(&s, &t, "Diagnostics failed", move |conn| {
        gathr::timeline::turns(conn, &chat)
    })
    .await
}

/// What `gathr diag trace` shows, for a reference with no turn behind it (a sign-in, a request).
async fn trace(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(reference): Path<String>,
) -> Answer {
    db(&s, &t, "Diagnostics failed", move |conn| {
        gathr::diag::trace(conn, &reference)
    })
    .await
}

#[derive(Deserialize)]
struct UiEvent {
    name: String,
    duration_ms: Option<f64>,
    error: Option<String>,
    #[serde(default)]
    attrs: serde_json::Map<String, Value>,
}

/// The window's own errors and timings, into the event record beside the service's (0019).
async fn ui_event(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<UiEvent>,
) -> Answer {
    if !body.name.starts_with("gathr.ui.") {
        let why = "a window event's name starts with gathr.ui.";
        return Err(fail(StatusCode::BAD_REQUEST, "Report failed", why, &t.0));
    }
    let mut event = Event::point(&body.name).trace(&t.0);
    event = match &body.error {
        Some(error) => event.error(error),
        None => event.ok(),
    };
    event.duration_us = body.duration_ms.map(|ms| (ms * 1000.0).round() as i64);
    event.attrs = body.attrs;
    s.writer.record_now(event).await;
    Ok(Json(json!({ "recorded": true })))
}

/// Claude Code itself, once for every account: where it was found, and Gathr's copy (0025).
async fn engine(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    let found = command::find(s.chats.copy());
    Json(json!({
        "found": found.as_ref().ok().map(|p| p.display().to_string()),
        "why_not_found": found.err(),
        "copy": s.updates.view(),
    }))
}

/// Checks the release feed now (0025); `GET /api/engines/claude` shows how it went.
async fn update_engine(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    if !s.updates.check_now() {
        let why = "this dev build has no release feed: set GATHR_CLAUDE_RELEASES";
        return Err(fail(StatusCode::CONFLICT, "Update failed", why, &t.0));
    }
    Ok(Json(json!({ "checking": true })))
}

/// Everything live, as JSON messages: deltas, messages, tools, turns and approvals.
async fn live(Extract(s): Extract<Arc<State>>, upgrade: WebSocketUpgrade) -> Response {
    let events = s.chats.live.subscribe();
    upgrade.on_upgrade(move |socket| forward(socket, events))
}

async fn forward(mut socket: WebSocket, mut events: tokio::sync::broadcast::Receiver<Value>) {
    use tokio::sync::broadcast::error::RecvError;
    loop {
        let next = tokio::select! {
            next = events.recv() => next,
            // Reading is what answers the window's Close (and its pings); it sends nothing else.
            frame = socket.recv() => match frame {
                // The reply to a Close is only queued; the next read sends it and ends the socket.
                Some(Ok(Message::Close(_))) => {
                    let _ = socket.recv().await;
                    return;
                }
                Some(Err(_)) | None => return,
                Some(Ok(_)) => continue,
            },
        };
        let event = match next {
            Ok(event) => event,
            // A client that fell behind is told, and reloads the chat it shows.
            Err(RecvError::Lagged(missed)) => json!({ "type": "lagged", "missed": missed }),
            Err(RecvError::Closed) => return,
        };
        if socket
            .send(Message::Text(event.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }
}

/// Who the message goes to among the chat's agents (0028), adding the one picked in a chat with no
/// project (F8b).
async fn addressee(
    s: &State,
    t: &Trace,
    chat: &str,
    sent_to: Option<String>,
    text: &str,
) -> Result<Option<String>, Fail> {
    let (id, text) = (chat.to_string(), text.to_string());
    let routed = s
        .writer
        .call(move |conn| gathr::team::route(conn, &id, sent_to.as_deref(), &text))
        .await;
    let routed = match routed {
        Some(Ok(routed)) => routed,
        Some(Err(e)) => {
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Send failed",
                &e.to_string(),
                &t.0,
            ));
        }
        None => {
            let why = "the database didn't answer";
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Send failed",
                why,
                &t.0,
            ));
        }
    };
    let routed = routed.map_err(|why| fail(StatusCode::BAD_REQUEST, "Send failed", why, &t.0))?;
    if let Some(members) = routed.joined {
        crate::team::changed(s, t, chat, members);
    }
    Ok(routed.agent)
}

pub async fn known(s: &State, t: &Trace, chat: &str) -> Result<(), Fail> {
    let id = chat.to_string();
    match s
        .writer
        .call(move |conn| store::chat_exists(conn, &id))
        .await
    {
        Some(Ok(true)) => Ok(()),
        Some(Ok(false)) => Err(fail(
            StatusCode::NOT_FOUND,
            "Chat failed",
            "no such chat",
            &t.0,
        )),
        _ => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Chat failed",
            "the database didn't answer",
            &t.0,
        )),
    }
}

/// Runs `work` on the database: its answer, a rule's refusal (409), or a failure.
pub async fn db(
    s: &State,
    t: &Trace,
    error: &'static str,
    work: impl FnOnce(&mut rusqlite::Connection) -> Result<Value, gathr::Error> + Send + 'static,
) -> Answer {
    match s.writer.call(work).await {
        Some(Ok(value)) => Ok(Json(value)),
        Some(Err(gathr::Error::Refused(why))) => Err(fail(StatusCode::CONFLICT, error, &why, &t.0)),
        Some(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            &e.to_string(),
            &t.0,
        )),
        None => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            "the database didn't answer",
            &t.0,
        )),
    }
}
