//! `GET /api/search` (plans/search.md): chat titles and message text, one event per request
//! without the query's words, as `diag bundle` is made for sharing.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Extension, Query, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::event::Event;
use gathr::search;
use serde::Deserialize;

use crate::api::{State, Trace};
use crate::routes::{Answer, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/search", get(find))
}

#[derive(Deserialize)]
struct Asked {
    #[serde(default)]
    q: String,
    #[serde(default = "yes")]
    archived: bool,
    #[serde(default = "thirty")]
    limit: usize,
}

fn yes() -> bool {
    true
}

fn thirty() -> usize {
    30
}

async fn find(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Query(asked): Query<Asked>,
) -> Answer {
    const ERROR: &str = "Search failed";
    let started = Instant::now();
    let q = search::clean(&asked.q);
    let event = Event::point("gathr.search.query")
        .trace(&t.0)
        .attr("chars", q.chars().count())
        .attr("words", search::words(&q).len())
        .attr("archived", asked.archived);
    let (archived, limit) = (asked.archived, asked.limit);
    let found = s
        .writer
        .call(move |conn| search::search(conn, &q, archived, limit))
        .await;
    let mut event = match &found {
        Some(Ok(found)) => event
            .ok()
            .attr("chat_hits", found.chat_hits)
            .attr("message_hits", found.message_hits),
        Some(Err(e)) => event.error("database").attr("reason", e.to_string()),
        None => event
            .error("database")
            .attr("reason", "the database didn't answer"),
    };
    event.duration_us = Some(started.elapsed().as_micros() as i64);
    s.writer.record_now(event).await;
    match found {
        Some(Ok(found)) => Ok(Json(found.answer)),
        Some(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            ERROR,
            &e.to_string(),
            &t.0,
        )),
        None => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            ERROR,
            "the database didn't answer",
            &t.0,
        )),
    }
}
