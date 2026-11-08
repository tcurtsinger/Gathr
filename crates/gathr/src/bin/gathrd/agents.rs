//! Settings > Agents (0026): list, add, change and delete agents. A change applies from each
//! agent's next turn; a turn under way keeps what it started with.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::agents::{self, Agent, Change, Refused};
use gathr::event::Event;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use crate::accounts::find as find_account;
use crate::api::{State, Trace};
use crate::models;
use crate::routes::{Answer, Fail, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/agents", get(list).post(add))
        .route("/api/agents/{agent}", get(one).patch(change).delete(remove))
}

/// What the API takes: each field left out stays, and `null` clears a model or effort, which then
/// falls back to the engine's own default.
#[derive(Deserialize)]
struct Body {
    name: Option<String>,
    account: Option<String>,
    #[serde(default, deserialize_with = "present")]
    model: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    effort: Option<Option<String>>,
}

/// Tells `"model": null` (clear it) from no `model` at all (keep it).
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

impl Body {
    fn change(self) -> Change {
        Change {
            name: self.name,
            account: self.account,
            model: self.model,
            effort: self.effort,
        }
    }
}

/// The agent, or a 404 that says so under `error`.
pub async fn find(s: &State, t: &Trace, id: &str, error: &'static str) -> Result<Agent, Fail> {
    let id = id.to_string();
    match s.writer.call(move |conn| agents::get(conn, &id)).await {
        Some(Ok(Some(agent))) => Ok(agent),
        Some(Ok(None)) => Err(fail(StatusCode::NOT_FOUND, error, "no such agent", &t.0)),
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

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    match s.writer.call(|conn| agents::list(conn)).await {
        Some(Ok(listed)) => {
            let shown: Vec<Value> = listed.iter().map(|a| shown(&s, a)).collect();
            Ok(Json(json!({ "agents": shown })))
        }
        Some(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Agents failed",
            &e.to_string(),
            &t.0,
        )),
        None => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Agents failed",
            "the database didn't answer",
            &t.0,
        )),
    }
}

async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    Ok(Json(shown(&s, &find(&s, &t, &id, "Agent failed").await?)))
}

/// The agent with its engine's hold, if a close it's waiting on has taken too long (0010).
fn shown(s: &State, agent: &Agent) -> Value {
    let mut shown = json!(agent);
    shown["hold"] = s.chats.hold(&agent.id);
    shown
}

async fn add(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Body>,
) -> Answer {
    const ERROR: &str = "Add failed";
    let blank = Agent {
        id: String::new(),
        name: String::new(),
        account: String::new(),
        model: None,
        effort: None,
    };
    let agent = blank.changed(body.change());
    save(&s, &t, ("add", ERROR), agent).await
}

async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Body>,
) -> Answer {
    const ERROR: &str = "Save failed";
    let agent = find(&s, &t, &id, ERROR).await?.changed(body.change());
    save(&s, &t, ("change", ERROR), agent).await
}

async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    const ERROR: &str = "Delete failed";
    let agent = find(&s, &t, &id, ERROR).await?;
    let gone = agent.id.clone();
    match s.writer.call(move |conn| agents::remove(conn, &gone)).await {
        Some(Ok(_)) => s.chats.retire(&agent.id),
        Some(Err(e)) => {
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                ERROR,
                &e.to_string(),
                &t.0,
            ));
        }
        None => {
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                ERROR,
                "the database didn't answer",
                &t.0,
            ));
        }
    }
    record(&s, &t, "remove", &agent);
    Ok(Json(json!({ "removed": true })))
}

/// Checks the model and effort against the account's engine, then writes the agent.
async fn save(
    s: &Arc<State>,
    t: &Trace,
    (change, error): (&'static str, &'static str),
    agent: Agent,
) -> Answer {
    let refused = |status, why: &str| Err(fail(status, error, why, &t.0));
    if agent.model.is_some() || agent.effort.is_some() {
        let account = match find_account(s, t, &agent.account, error).await {
            Ok(account) => account,
            Err(_) => return refused(StatusCode::BAD_REQUEST, Refused::NoAccount.why()),
        };
        let offered = models::list(s, &t.0, &account)
            .await
            .map_err(|why| format!("the account's models weren't listed: {why}"))
            .and_then(|list| {
                models::offered(&list, agent.model.as_deref(), agent.effort.as_deref())
            });
        if let Err(why) = offered {
            return refused(StatusCode::BAD_REQUEST, &why);
        }
    }
    let chats = s.chats.clone();
    let adding = change == "add";
    let saved = s
        .writer
        .call(move |conn| {
            // Read in the write's own call, so a removal that began first is always seen.
            let removing = chats.being_removed();
            match adding {
                true => agents::add(conn, &agent, &removing),
                false => agents::update(conn, &agent, &removing),
            }
        })
        .await;
    let agent = match saved {
        Some(Ok(Ok(agent))) => agent,
        Some(Ok(Err(why @ Refused::Taken))) => return refused(StatusCode::CONFLICT, why.why()),
        Some(Ok(Err(why @ Refused::Removing))) => return refused(StatusCode::CONFLICT, why.why()),
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
    record(s, t, change, &agent);
    Ok(Json(json!(agent)))
}

/// Records the change and tells every window.
fn record(s: &State, t: &Trace, change: &'static str, agent: &Agent) {
    let event = Event::point("gathr.agent.change")
        .trace(&t.0)
        .ok()
        .attr("agent", agent.id.clone())
        .attr("account", agent.account.clone())
        .attr("model", agent.model.clone())
        .attr("effort", agent.effort.clone())
        .attr("change", change);
    s.writer.record(event);
    s.chats.publish(json!({ "type": "agents" }));
}
