//! A chat's board, for the window: its lines and how many agents the chat has. Agents set the lines
//! (`tools/board.rs`), and each change is sent live as `board`.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Extension, Path, State as Extract};
use axum::routing::get;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/chats/{chat}/board", get(board))
}

async fn board(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    db(&s, &t, "Board failed", move |conn| {
        let agents: i64 = conn.query_row(
            "SELECT count(*) FROM chat_members WHERE chat_id = ?1",
            [&chat],
            |r| r.get(0),
        )?;
        Ok(json!({ "lines": gathr::board::lines(conn, &chat)?, "agents": agents }))
    })
    .await
}
