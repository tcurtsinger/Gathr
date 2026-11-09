//! Settings > Engines' accounts (0026): list, add, rename and remove. Signing in and out is in
//! [`crate::sign_in`], and each account's models in [`crate::models`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::accounts::{self, Account, Refused};
use gathr::agents;
use gathr::claude::{command, login};
use gathr::event::Event;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, fail};
use crate::sign_in::{log_out_held, pass_gate, record_as};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/accounts", get(list).post(add))
        .route(
            "/api/accounts/{account}",
            get(one).patch(rename).delete(remove),
        )
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

/// Signs the account out, deletes its folder and forgets it. Chats on it move to the next
/// account first, and its engines close the gentle way (0002).
async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    const ERROR: &str = "Remove failed";
    let account = find(&s, &t, &id, ERROR).await?;
    // Marked first: an agent saved from here on is refused, so none is missed below.
    s.chats.removing(&account.id, true).await;
    let looked = account.id.clone();
    let using = match s
        .writer
        .call(move |conn| agents::on_account(conn, &looked))
        .await
    {
        Some(Ok(names)) => names,
        Some(Err(e)) => vec![format!("(agents weren't listed: {e})")],
        None => vec!["(agents weren't listed: the database didn't answer)".into()],
    };
    if !using.is_empty() {
        s.chats.removing(&account.id, false).await;
        let why = format!("agents use it: {}", using.join(", "));
        let event = Event::point("gathr.account.change")
            .trace(&t.0)
            .error("in_use")
            .attr("account", account.id.clone())
            .attr("change", "remove")
            .attr("why", why.clone());
        s.writer.record(event);
        return Err(fail(StatusCode::CONFLICT, ERROR, &why, &t.0));
    }
    s.sign_ins.get(&account.id).cancel().await;
    let done = let_go(&s, &t.0, &account).await;
    match &done {
        Ok(()) => s.chats.removed(&account.id),
        Err(_) => s.chats.removing(&account.id, false).await,
    }
    let mut event = Event::point("gathr.account.change")
        .trace(&t.0)
        .attr("account", account.id.clone())
        .attr("engine", account.engine.clone())
        .attr("change", "remove");
    event = match &done {
        Ok(()) => event.ok(),
        Err(why) => event.error("remove").attr("why", why.clone()),
    };
    s.writer.record(event);
    s.chats.publish(json!({ "type": "accounts" }));
    match done {
        Ok(()) => Ok(Json(json!({ "removed": true }))),
        Err(why) => Err(fail(StatusCode::INTERNAL_SERVER_ERROR, ERROR, &why, &t.0)),
    }
}

async fn let_go(s: &State, trace: &str, account: &Account) -> Result<(), String> {
    let folder = account.folder(&s.data);
    s.chats.leave(&folder).await;
    // Held to the end, so nothing starts on the account between its sign-out and its removal.
    let _gate = if folder.exists() {
        let started = Instant::now();
        let done = sign_out(s, &folder).await;
        let why = done.as_ref().err().map(String::as_str);
        record_as(s, "sign_out", &account.id, trace, why, started).await;
        Some(done?)
    } else {
        // Never signed in here, so there's no login to end.
        None
    };
    delete_folder(&s.data.path().join("accounts").join(&account.id)).await?;
    let id = account.id.clone();
    match s.writer.call(move |conn| accounts::remove(conn, &id)).await {
        Some(Ok(_)) => Ok(()),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

/// Ends the login before its folder goes, with the gate held to the end: `claude auth logout`,
/// which also clears its macOS Keychain item. One that reads signed out has nothing to end, so it
/// goes without Claude Code (F11).
async fn sign_out(s: &State, folder: &std::path::Path) -> Result<login::Hold, String> {
    let gate = pass_gate(s, folder).await?;
    if s.chats.login_status(folder).await.0 != login::Login::SignedOut {
        let program = command::find(s.chats.copy())?;
        log_out_held(s, &program, folder).await?;
    }
    Ok(gate)
}

/// A process just closed may still hold its files for a moment on Windows: tried a few times.
async fn delete_folder(path: &std::path::Path) -> Result<(), String> {
    let mut tries = 0;
    loop {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) if tries == 5 => return Err(format!("{} wasn't deleted: {e}", path.display())),
            Err(_) => tries += 1,
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
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
    if change == "add" {
        s.chats.pick_account().await;
    }
    s.chats.publish(json!({ "type": "accounts" }));
    Ok(Json(view(s, &account).await))
}
