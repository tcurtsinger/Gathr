//! Tools on the web for the Plugins screen (`gathr::web_tools`). Each one's state and actions are
//! asked from it when shown; nothing of it is kept but its row. No answer carries a key or a sign-in:
//! at most whether one is set. Each change is sent live as `tools`. A tool on this PC is answered by
//! `pc_tools` where it differs.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::tool_sign_in;
use gathr::web_tools::{self, Access, Problem, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db, fail};
use crate::tools::relay;

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/tools", get(list).post(add))
        .route("/api/tools/{id}", get(one).patch(change).delete(remove))
}

/// The state a tool is in, worked out by connecting to it: none for one disabled.
fn state(answer: &Result<web_tools::Remote, Problem>) -> &'static str {
    match answer {
        Ok(_) => "connected",
        Err(Problem::NotReachable) => "not_reachable",
        Err(Problem::SignIn) => "sign_in",
        // Saved without a key: an agent added it, for the person to give one.
        Err(Problem::NeedsKey) => "needs_key",
        Err(Problem::KeyRefused) => "key_refused",
    }
}

/// What a tool is connected with, as read: `Err` when it was sealed but can't be unsealed (the data
/// moved to another PC or user, the Keychain item gone), which shows that tool as key refused, or
/// as to sign in again.
type Key = Result<Access, ()>;

/// The state of a tool whose key or sign-in can't be read.
fn unread(tool: &Tool) -> Problem {
    match tool.signed_in {
        true => Problem::SignIn,
        false => Problem::KeyRefused,
    }
}

/// Every tool with its key or sign-in, read for connecting; they stay in this process. One that
/// can't be read never fails the others.
async fn saved(s: &State, t: &Trace) -> Result<Vec<(Tool, Key)>, crate::routes::Fail> {
    let writer = s.writer.clone();
    let read = s.writer.call(move |conn| {
        let tools = web_tools::list(conn)?;
        Ok::<_, gathr::Error>(
            tools
                .into_iter()
                .map(|tool| {
                    let key = web_tools::access(conn, &tool.id, &writer).map_err(|_| ());
                    (tool, key)
                })
                .collect::<Vec<_>>(),
        )
    });
    match read.await {
        Some(Ok(tools)) => Ok(tools),
        Some(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Tools failed",
            &e.to_string(),
            &t.0,
        )),
        None => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Tools failed",
            "the database didn't answer",
            &t.0,
        )),
    }
}

/// Every tool, each enabled one's state asked of it at once, within `web_tools::STATE`.
async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let mut asked = tokio::task::JoinSet::new();
    for (i, (tool, key)) in saved(&s, &t).await?.into_iter().enumerate() {
        let s = s.clone();
        asked.spawn(async move {
            let state = match (tool.enabled, key) {
                (false, _) => None,
                (true, _) if tool.kind == "pc" => {
                    let copy = crate::pc_tools::copy(&s, &tool.id);
                    Some(crate::pc_tools::state(&copy.phase(web_tools::STATE).await))
                }
                (true, Err(())) => Some(state(&Err(unread(&tool)))),
                (true, Ok(key)) => Some(state(
                    &web_tools::connect(&tool.link, &key, web_tools::STATE).await,
                )),
            };
            let mut shown = json!(tool);
            shown["state"] = json!(state);
            (i, shown)
        });
    }
    let mut tools = asked.join_all().await;
    tools.sort_by_key(|(i, _)| *i);
    let tools: Vec<Value> = tools.into_iter().map(|(_, tool)| tool).collect();
    Ok(Json(json!({ "tools": tools })))
}

/// One tool, with its state and, when it's connected, its actions; one with a Client ID, with the
/// address its admin gives the app they made.
async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let found = saved(&s, &t)
        .await?
        .into_iter()
        .find(|(tool, _)| tool.id == id);
    let Some((tool, key)) = found else {
        return Err(fail(
            StatusCode::CONFLICT,
            "Tool failed",
            &format!("There's no tool {id}"),
            &t.0,
        ));
    };
    if tool.kind == "pc" {
        return crate::pc_tools::one(&s, &t, tool).await;
    }
    let connected = match key {
        Ok(key) => web_tools::connect(&tool.link, &key, web_tools::CONNECT).await,
        Err(()) => Err(unread(&tool)),
    };
    let actions = match &connected {
        Ok(remote) => web_tools::actions(remote, web_tools::CONNECT).await.ok(),
        Err(_) => None,
    };
    let state = match (&connected, &actions) {
        (Ok(_), None) => "not_reachable",
        _ => state(&connected),
    };
    let mut shown = json!(tool);
    shown["state"] = json!(state);
    shown["actions"] = json!(actions);
    if tool.client_id.is_some() {
        shown["redirect"] = json!(crate::tool_sign_in::redirect(&s));
    }
    Ok(Json(shown))
}

/// A tool on the web by its `link`, or one on this PC by the `text` pasted from its instructions.
#[derive(Deserialize)]
struct New {
    link: Option<String>,
    text: Option<String>,
    name: Option<String>,
    key: Option<String>,
}

/// Connects to the link, then saves it if it works. A tool that wants a key, or signs in through the
/// browser, is answered with what it needs and nothing is saved. Pasted text adds a tool on this PC.
async fn add(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(new): Json<New>,
) -> Answer {
    if let Some(text) = &new.text {
        return crate::pc_tools::add(&s, &t, text, new.name).await;
    }
    let refused = |why: &str| Err(fail(StatusCode::CONFLICT, "Add failed", why, &t.0));
    let link = new.link.unwrap_or_default().trim().to_string();
    let host = match web_tools::checked(&link) {
        Ok(host) => host,
        Err(e) => return refused(&e.to_string()),
    };
    let key = new
        .key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    let access = key.clone().map_or(Access::Open, Access::Key);
    let remote = match web_tools::connect(&link, &access, web_tools::CONNECT).await {
        Ok(remote) => remote,
        Err(Problem::NeedsKey) => return Ok(Json(json!({ "needs": "key" }))),
        Err(Problem::SignIn) => {
            // Where the service takes no app it doesn't know, the admin makes one for Gathr.
            return Ok(Json(match tool_sign_in::registers(&link).await {
                Ok(false) => json!({
                    "needs": "sign_in",
                    "registers": false,
                    "redirect": crate::tool_sign_in::redirect(&s),
                }),
                _ => json!({ "needs": "sign_in" }),
            }));
        }
        Err(Problem::KeyRefused) => return refused("The key was refused."),
        Err(Problem::NotReachable) => return refused("No tool at this link."),
    };
    let given = new
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    let name = given
        .or_else(|| web_tools::own_name(&remote))
        .unwrap_or(host);
    drop(remote);
    let answer = db(&s, &t, "Add failed", move |conn| {
        Ok(json!({ "tool": web_tools::add(conn, &name, &link, key.as_deref())? }))
    })
    .await;
    told(&s, answer)
}

#[derive(Deserialize)]
struct Change {
    enabled: Option<bool>,
    key: Option<String>,
    /// For a tool on this PC: what it runs, pasted again, and its name.
    text: Option<String>,
    name: Option<String>,
}

/// Enables or disables it, or gives it a new key, saved only when the tool takes it. A tool on this
/// PC takes what it runs, pasted again, and a name.
async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(change): Json<Change>,
) -> Answer {
    if change.text.is_some() || change.name.is_some() {
        return crate::pc_tools::change(&s, &t, id, change.text, change.name).await;
    }
    let key = change.key.map(|k| k.trim().to_string());
    if let Some(key) = &key {
        let found = saved(&s, &t)
            .await?
            .into_iter()
            .find(|(tool, _)| tool.id == id);
        let Some((tool, _)) = found else {
            return Err(fail(
                StatusCode::CONFLICT,
                "Save failed",
                &format!("There's no tool {id}"),
                &t.0,
            ));
        };
        let access = Access::Key(key.clone());
        let why = match web_tools::connect(&tool.link, &access, web_tools::CONNECT).await {
            Ok(_) => None,
            Err(Problem::NotReachable) => Some("The tool can't be reached."),
            Err(_) => Some("The key was refused."),
        };
        if let Some(why) = why {
            return Err(fail(StatusCode::CONFLICT, "Save failed", why, &t.0));
        }
    }
    let answer = db(&s, &t, "Save failed", move |conn| {
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            web_tools::set_key(conn, &id, &key)?;
        }
        if let Some(on) = change.enabled {
            web_tools::set_enabled(conn, &id, on)?;
        }
        relay::forget(&id);
        Ok(json!(web_tools::get(conn, &id)?))
    })
    .await;
    told(&s, answer)
}

/// Removes the tool, its key and its sign-in.
async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let answer = db(&s, &t, "Remove failed", move |conn| {
        web_tools::remove(conn, &id)?;
        relay::forget(&id);
        Ok(json!({ "removed": id }))
    })
    .await;
    told(&s, answer)
}

/// An open window is told when a change was saved.
pub fn told(s: &State, answer: Answer) -> Answer {
    if answer.is_ok() {
        let _ = s.chats.live.send(json!({ "type": "tools" }));
    }
    answer
}
