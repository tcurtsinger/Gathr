//! The chat's Background tab (0084): a command's output, Stop on one still running, and Clear for
//! the ended ones.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::api::{State, Trace};
use crate::chats::Cmd;
use crate::routes::{Answer, db, fail, known};

/// How much of a command's output the tab shows: its end.
const TAIL: u64 = 64 * 1024;

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/background", delete(clear))
        .route("/api/chats/{chat}/background/{task}/output", get(output))
        .route("/api/chats/{chat}/background/{task}/stop", post(stop))
}

/// The end of the command's output, from the file the engine wrote it to: `{text, cut}`, where cut
/// says its start was left out; `text` is null once the file is gone.
async fn output(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, task)): Path<(String, String)>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let Json(file) = db(&s, &t, "Output failed", move |conn| {
        Ok(json!(gathr::background::output_of(conn, &chat, &task)?))
    })
    .await?;
    let Some(file) = file.as_str().map(str::to_string) else {
        return Ok(Json(json!({ "text": null, "cut": false })));
    };
    let read = tokio::task::spawn_blocking(move || tail(std::path::Path::new(&file))).await;
    Ok(Json(match read {
        Ok(Ok((text, cut))) => json!({ "text": text, "cut": cut }),
        _ => json!({ "text": null, "cut": false }),
    }))
}

/// The file's last `TAIL` bytes as text, from a line's start, and whether any came before them.
fn tail(file: &std::path::Path) -> std::io::Result<(String, bool)> {
    let mut f = std::fs::File::open(file)?;
    let size = f.metadata()?.len();
    let cut = size > TAIL;
    f.seek(SeekFrom::Start(size.saturating_sub(TAIL)))?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = match (cut, text.find('\n')) {
        (true, Some(at)) => text[at + 1..].to_string(),
        _ => text.into_owned(),
    };
    Ok((text, cut))
}

/// Stop on a running command: the engine's own stop, by the agent whose engine runs it. It ends
/// as the engine tells, as any other.
async fn stop(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, task)): Path<(String, String)>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (c, id) = (chat.clone(), task.clone());
    let Json(agent) = db(&s, &t, "Stop failed", move |conn| {
        Ok(match gathr::background::running_in(conn, &c, &id)? {
            Some(agent) => json!({ "agent": agent }),
            None => json!(null),
        })
    })
    .await?;
    if agent.is_null() {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "Stop failed",
            "it isn't running",
            &t.0,
        ));
    }
    let agent = agent["agent"].as_str().map(str::to_string);
    let _ = s
        .chats
        .task(&chat, agent.as_deref())
        .send(Cmd::StopTask { chat, task });
    Ok(Json(json!({ "stopping": true })))
}

/// Clear: the chat's ended commands go; the running ones stay.
async fn clear(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let id = chat.clone();
    let Json(left) = db(&s, &t, "Clear failed", move |conn| {
        gathr::background::clear_ended(conn, &id)?;
        Ok(json!(gathr::background::in_chat(conn, &id)?))
    })
    .await?;
    s.chats
        .publish(json!({ "type": "background", "chat": chat, "background": left }));
    Ok(Json(json!({ "cleared": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_starts_at_a_line_and_says_it_was_cut() {
        let dir = std::env::temp_dir().join(format!("gathr-tail-{}", gathr::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let short = dir.join("short.output");
        std::fs::write(&short, "one\ntwo\n").unwrap();
        assert_eq!(tail(&short).unwrap(), ("one\ntwo\n".to_string(), false));
        let long = dir.join("long.output");
        let line = "x".repeat(99) + "\n";
        std::fs::write(&long, line.repeat(1000)).unwrap();
        let (text, cut) = tail(&long).unwrap();
        assert!(cut);
        assert!(text.starts_with('x') && text.len() <= TAIL as usize && text.len() % 100 == 0);
    }
}
