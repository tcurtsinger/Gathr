//! A chat's Terminal tabs' socket (`gathr::terminal`): the chat's terminals and what each prints to
//! the window, as binary frames led by the terminal's id; the person's keys, sizes and tab actions
//! back. Guarded as `/api/live` is.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Path, State as Extract};
use axum::response::Response;
use axum::routing::get;
use gathr::terminal::{Info, Out, Terminals};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::api::{State, Trace};
use crate::routes::Fail;

/// How the socket's path ends, after `/api/chats/{chat}`.
pub const END: &str = "/terminals";

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/chats/{chat}/terminals", get(socket))
}

async fn socket(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, Fail> {
    folders(&s, &t, &chat).await?;
    Ok(upgrade.on_upgrade(move |socket| serve(socket, s, t, chat)))
}

/// Where a terminal of this chat may start, the one a new one starts in first: the project's
/// folder, or the home folder for a chat without one, then the agents' worktrees.
pub async fn folders(s: &State, t: &Trace, chat: &str) -> Result<Vec<PathBuf>, Fail> {
    let tree = crate::files::tree_of(s, t, chat, "Terminal failed").await?;
    let first = tree.project.as_ref().map(|p| p.path.clone());
    let first = first.or_else(std::env::home_dir);
    let agents = tree.agents.iter().map(|p| p.path.clone());
    Ok(first.into_iter().chain(agents).collect())
}

fn listed(terminals: &Terminals, chat: &str) -> Message {
    let list: Vec<Info> = terminals.list(chat);
    Message::Text(
        json!({ "type": "list", "terminals": list })
            .to_string()
            .into(),
    )
}

fn printed(id: &str, bytes: &[u8]) -> Message {
    Message::Binary([id.as_bytes(), bytes].concat().into())
}

async fn serve(mut socket: WebSocket, s: Arc<State>, t: Trace, chat: String) {
    let terminals = s.terminals.clone();
    let mut out = terminals.watch();
    let mut first = vec![listed(&terminals, &chat)];
    for info in terminals.list(&chat) {
        first.push(printed(&info.id, &terminals.kept(&info.id)));
    }
    for message in first {
        if socket.send(message).await.is_err() {
            return;
        }
    }
    loop {
        tokio::select! {
            sent = out.recv() => {
                let message = match sent {
                    Ok(Out::Listed(c)) if *c == *chat => listed(&terminals, &chat),
                    Ok(Out::Printed(c, id, bytes)) if *c == *chat => printed(&id, &bytes),
                    Ok(_) => continue,
                    // A window that fell behind is sent the list and each terminal's output again.
                    Err(RecvError::Lagged(_)) => {
                        let again = json!({ "type": "again" }).to_string();
                        if socket.send(Message::Text(again.into())).await.is_err() {
                            break;
                        }
                        let mut all = vec![listed(&terminals, &chat)];
                        for info in terminals.list(&chat) {
                            all.push(printed(&info.id, &terminals.kept(&info.id)));
                        }
                        let mut failed = false;
                        for message in all {
                            failed |= socket.send(message).await.is_err();
                        }
                        if failed {
                            break;
                        }
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };
                if socket.send(message).await.is_err() {
                    break;
                }
            },
            said = socket.recv() => match said {
                Some(Ok(Message::Text(text))) => {
                    let Ok(said) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    if let Err(why) = act(&s, &t, &chat, &said).await {
                        let failed = json!({ "type": "failed", "why": why }).to_string();
                        if socket.send(Message::Text(failed.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
}

/// What the window asked, done.
async fn act(s: &State, t: &Trace, chat: &str, said: &Value) -> Result<(), String> {
    let terminals = &s.terminals;
    let id = said["id"].as_str().unwrap_or_default();
    // Only this chat's terminals take what its window says.
    let ours = || terminals.info(id).is_some_and(|i| i.chat == chat);
    match said["type"].as_str().unwrap_or_default() {
        "open" => {
            let allowed = folders(s, t, chat)
                .await
                .map_err(|_| "No folder".to_string())?;
            let asked = said["folder"].as_str().map(PathBuf::from);
            // A folder in one of them: the folder an agent asked the person to run a command in.
            let folder = match asked {
                Some(asked) => allowed
                    .iter()
                    .any(|f| asked.starts_with(f))
                    .then_some(asked),
                None => allowed.into_iter().next(),
            };
            let folder = folder.ok_or("That folder isn't one of this chat's")?;
            let vars = gathr::run::fresh().await;
            let (shared, chat) = (terminals.clone(), chat.to_string());
            let opened =
                tokio::task::spawn_blocking(move || shared.open(&chat, &folder, None, None, vars));
            let id = opened
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            // A command an agent asked the person to run, typed in: run at once, or left for their Enter.
            if let Some(typed) = said["typed"].as_str() {
                let enter = if said["run"] == true { "\r" } else { "" };
                let typed = format!("{typed}{enter}");
                terminals
                    .input(&id, typed.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
        "again" if ours() => {
            let vars = gathr::run::fresh().await;
            terminals.again(id, vars).map_err(|e| e.to_string())?;
        }
        "input" if ours() => {
            let data = said["data"].as_str().unwrap_or_default();
            terminals
                .input(id, data.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        "resize" if ours() => {
            let size = |k: &str| said[k].as_u64().unwrap_or(0).min(u16::MAX.into()) as u16;
            terminals.resize(id, size("cols"), size("rows"));
        }
        "stop" if ours() => terminals.stop(id),
        "close" if ours() => terminals.close(id),
        _ => {}
    }
    Ok(())
}
