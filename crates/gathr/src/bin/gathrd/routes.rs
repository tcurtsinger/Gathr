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
use crate::chats::{Cmd, PATH_SETTING};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats", get(list).post(create))
        .route("/api/chats/{chat}", get(history))
        .route("/api/chats/{chat}/messages", post(send))
        .route("/api/chats/{chat}/stop", post(stop))
        .route("/api/chats/{chat}/approvals/{request}", post(approve))
        .route("/api/chats/{chat}/turns", get(turns))
        .route("/api/ui/events", post(ui_event))
        .route("/api/engines/claude", get(engine).put(set_engine))
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

#[derive(Deserialize)]
struct NewChat {
    title: Option<String>,
}

async fn create(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<NewChat>,
) -> Answer {
    let title = body.title.unwrap_or_else(|| store::UNTITLED.into());
    db(&s, &t, "New chat failed", move |conn| {
        store::create_chat(conn, &title)
    })
    .await
}

async fn history(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (reply, open) = oneshot::channel();
    let _ = s.chats.task(&chat).send(Cmd::Approvals { reply });
    let Json(mut history) = db(&s, &t, "Chat failed", move |conn| {
        store::history(conn, &chat)
    })
    .await?;
    history["approvals"] = open.await.unwrap_or_else(|_| json!([]));
    Ok(Json(history))
}

#[derive(Deserialize)]
struct NewMessage {
    text: String,
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
    let (reply, answer) = oneshot::channel();
    let command = Cmd::Send {
        text: body.text,
        trace: t.0.clone(),
        reply,
    };
    let _ = s.chats.task(&chat).send(command);
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
    let (reply, answer) = oneshot::channel();
    let _ = s.chats.task(&chat).send(Cmd::Stop { reply });
    answer.await.map(Json).map_err(|_| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Stop failed",
            "the chat's task ended",
            &t.0,
        )
    })
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
    let (reply, answer) = oneshot::channel();
    let command = Cmd::Approve {
        request_id: request,
        allow: body.allow,
        reply,
    };
    let _ = s.chats.task(&chat).send(command);
    match answer.await {
        Ok(true) => Ok(Json(json!({ "answered": true }))),
        _ => Err(fail(
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

/// Claude Code's path (set, and where it was found) and the account's sign-in state.
async fn engine(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    let setting = s.chats.path_setting().await;
    let found = command::find(setting.as_deref());
    let folder = s.chats.account_folder();
    let (state, expires) = s.chats.login_status().await;
    Json(json!({
        "path": setting,
        "found": found.as_ref().ok().map(|p| p.display().to_string()),
        "why_not_found": found.err(),
        "account": { "folder": folder, "login": state, "expires": expires.map(|ms| gathr::ids::utc(ms * 1000)) },
        "sign_in": s.sign_in.shown(),
    }))
}

#[derive(Deserialize)]
struct EngineSettings {
    path: Option<String>,
}

/// Sets Claude Code's path; the next start uses it, with no restart (S26).
async fn set_engine(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<EngineSettings>,
) -> Answer {
    let path = body.path.filter(|p| !p.trim().is_empty());
    db(&s, &t, "Save failed", move |conn| {
        store::set_setting(conn, PATH_SETTING, path.as_deref())?;
        Ok(json!({ "path": path }))
    })
    .await
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

async fn known(s: &State, t: &Trace, chat: &str) -> Result<(), Fail> {
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

async fn db(
    s: &State,
    t: &Trace,
    error: &'static str,
    work: impl FnOnce(&mut rusqlite::Connection) -> Result<Value, gathr::Error> + Send + 'static,
) -> Answer {
    match s.writer.call(work).await {
        Some(Ok(value)) => Ok(Json(value)),
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
