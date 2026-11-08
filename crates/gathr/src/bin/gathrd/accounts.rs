//! Settings > Engines' accounts (0026): list, add and rename. Signing in and out is in
//! [`crate::sign_in`], and each account's models in [`crate::models`].

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::accounts::{self, Account, Refused};
use gathr::event::Event;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/accounts", get(list).post(add))
        .route("/api/accounts/{account}", get(one).patch(rename))
}

/// The account, or a 404 that says so under `error`.
pub async fn find(s: &State, t: &Trace, id: &str, error: &'static str) -> Result<Account, Fail> {
    let id = id.to_string();
    match s.writer.call(move |conn| accounts::get(conn, &id)).await {
        Some(Ok(Some(account))) => Ok(account),
        Some(Ok(None)) => Err(fail(StatusCode::NOT_FOUND, error, "no such account", &t.0)),
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

/// What Settings shows for an account: its sign-in state, and a sign-in under way.
async fn view(s: &State, account: &Account) -> Value {
    let (login, expires) = s.chats.login_status(&account.folder(&s.data)).await;
    json!({
        "id": account.id,
        "name": account.name,
        "engine": account.engine,
        "login": login,
        "expires": expires.map(|ms| gathr::ids::utc(ms * 1000)),
        "sign_in": s.sign_ins.get(&account.id).shown(),
    })
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let listed = match s.writer.call(|conn| accounts::list(conn)).await {
        Some(Ok(listed)) => listed,
        Some(Err(e)) => {
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Accounts failed",
                &e.to_string(),
                &t.0,
            ));
        }
        None => {
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Accounts failed",
                "the database didn't answer",
                &t.0,
            ));
        }
    };
    let mut shown = Vec::with_capacity(listed.len());
    for account in &listed {
        shown.push(view(&s, account).await);
    }
    Ok(Json(json!({ "accounts": shown })))
}

async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let account = find(&s, &t, &id, "Account failed").await?;
    Ok(Json(view(&s, &account).await))
}

#[derive(Deserialize)]
struct NewAccount {
    engine: String,
    name: String,
}

async fn add(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<NewAccount>,
) -> Answer {
    let added = s
        .writer
        .call(move |conn| accounts::add(conn, &body.engine, &body.name))
        .await;
    changed(&s, &t, ("add", "Add failed"), added).await
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

async fn rename(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Name>,
) -> Answer {
    let renamed = s
        .writer
        .call(move |conn| accounts::rename(conn, &id, &body.name))
        .await;
    changed(&s, &t, ("rename", "Rename failed"), renamed).await
}

/// Answers an add or rename, records it, and tells every window.
async fn changed(
    s: &State,
    t: &Trace,
    (change, error): (&'static str, &'static str),
    done: Option<accounts::Changed>,
) -> Answer {
    let refused = |status, why: &str| Err(fail(status, error, why, &t.0));
    let account = match done {
        Some(Ok(Ok(account))) => account,
        Some(Ok(Err(why @ Refused::Taken))) => return refused(StatusCode::CONFLICT, why.why()),
        Some(Ok(Err(why @ Refused::Unknown))) => return refused(StatusCode::NOT_FOUND, why.why()),
        Some(Ok(Err(why))) => return refused(StatusCode::BAD_REQUEST, why.why()),
        Some(Err(e)) => return refused(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        None => {
            return refused(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the database didn't answer",
            );
        }
    };
    let event = Event::point("gathr.account.change")
        .trace(&t.0)
        .ok()
        .attr("account", account.id.clone())
        .attr("engine", account.engine.clone())
        .attr("change", change);
    s.writer.record(event);
    s.chats.publish(json!({ "type": "accounts" }));
    Ok(Json(view(s, &account).await))
}
