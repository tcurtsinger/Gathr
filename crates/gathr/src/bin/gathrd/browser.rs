//! The Browser tab's socket: the tabs' state and the shown page's live picture to the window, the
//! person's mouse, keys and tab actions back (`gathr::browser`). Guarded as `/api/live` is.

use std::sync::Arc;

use axum::Router;
use axum::extract::State as Extract;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use gathr::browser::{Browser, Out};
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

use crate::api::State;

pub const PATH: &str = "/api/browser";

pub fn routes() -> Router<Arc<State>> {
    Router::new().route(PATH, get(socket))
}

async fn socket(Extract(s): Extract<Arc<State>>, upgrade: WebSocketUpgrade) -> Response {
    let browser = s.browser.clone();
    upgrade.on_upgrade(move |socket| serve(socket, browser))
}

async fn serve(mut socket: WebSocket, browser: Browser) {
    let client = browser.client();
    let (state, mut out) = browser.watch();
    // Frames go only to a window that shows the page.
    let mut visible = false;
    let mut next = Some(state);
    loop {
        if let Some(sent) = next.take() {
            let message = match sent {
                Out::Text(text) => Message::Text(text.as_ref().into()),
                Out::Frame(_) if !visible => continue,
                Out::Frame(frame) => Message::Binary(frame),
            };
            if socket.send(message).await.is_err() {
                break;
            }
        }
        tokio::select! {
            sent = out.recv() => match sent {
                Ok(sent) => next = Some(sent),
                // A window that fell behind skips frames, and is sent the state as it is now.
                Err(RecvError::Lagged(_)) => next = Some(browser.watch().0),
                Err(RecvError::Closed) => break,
            },
            said = socket.recv() => match said {
                Some(Ok(Message::Text(text))) => {
                    let Ok(said) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    if said["type"] == "view" {
                        visible = said["visible"].as_bool().unwrap_or(false);
                    }
                    browser.said(client, said);
                }
                Some(Ok(Message::Close(_))) => {
                    let _ = socket.recv().await;
                    break;
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
        }
    }
    browser.left(client);
}
