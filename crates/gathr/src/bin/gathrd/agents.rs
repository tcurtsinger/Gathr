//! Settings > Agents (0026): list, add, change and delete agents. A change applies from each
//! agent's next turn; a turn under way keeps what it started with.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::agents::{self, Agent, Approvals, Change, Refused};
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
        .route(
            "/api/agents/{agent}",
            get(one).patch(change).delete(crate::deletes::agent),
        )
        .route("/api/proposals/{message}/add", post(add_proposed))
}

/// What the API takes: each field left out stays, and `null` clears a model or effort, which then
/// falls back to the engine's own default, or a picture's shape, eyes or colour, which the id then
/// picks, or its description.
#[derive(Deserialize, Default)]
struct Body {
    name: Option<String>,
    account: Option<String>,
    #[serde(default, deserialize_with = "present")]
    model: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    effort: Option<Option<String>>,
    approvals: Option<Approvals>,
    #[serde(default, deserialize_with = "present")]
    shape: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    eyes: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    colour: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    description: Option<Option<String>>,
}

/// Tells `"model": null` (clear it) from no `model` at all (keep it).
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

impl Body {
    fn change(self) -> Change {
        Change {
            name: self.name,
            account: self.account,
            model: self.model,
            effort: self.effort,
            approvals: self.approvals,
            shape: self.shape,
            eyes: self.eyes,
            colour: self.colour,
            description: self.description,
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
    let listed = s
        .writer
        .call(|conn| Ok::<_, gathr::Error>((agents::list(conn)?, agents::removed(conn)?)));
    match listed.await {
        // The removed ones apart, for the names on their old messages (0069).
        Some(Ok((listed, removed))) => {
            let shown: Vec<Value> = listed.iter().map(|a| shown(&s, a)).collect();
            Ok(Json(json!({ "agents": shown, "removed": removed })))
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
        approvals: Approvals::default(),
        shape: None,
        eyes: None,
        colour: None,
        description: None,
    };
    let agent = blank.changed(body.change());
    save(&s, &t, ("add", ERROR), agent, None).await
}

async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Body>,
) -> Answer {
    change_as(&s, &t, &id, body.change(), None).await
}

/// A change by the person (`by` none) or by an agent through its settings tool (0043 C5a).
pub async fn change_as(
    s: &Arc<State>,
    t: &Trace,
    id: &str,
    change: Change,
    by: Option<String>,
) -> Answer {
    const ERROR: &str = "Save failed";
    let agent = find(s, t, id, ERROR).await?.changed(change.clone());
    save(s, t, ("change", ERROR), agent, Some((change, by))).await
}

/// Add agent, from the card of an agent the assistant proposed (`propose_agent`): as proposed, or with what the Agents
/// screen's form for a new agent says. Added as `add` adds one, and the proposal marked added with it; once only.
async fn add_proposed(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(message): Path<String>,
    body: Option<Json<Body>>,
) -> Answer {
    const ERROR: &str = "Add failed";
    let id = message.clone();
    let found = s
        .writer
        .call(move |c| gathr::proposals::find_agent(c, &id))
        .await;
    let Some(p) = crate::team::written(&t, found, ERROR)? else {
        return Err(fail(StatusCode::NOT_FOUND, ERROR, "no such proposal", &t.0));
    };
    if p.added.is_some() {
        let why = "This agent was added already.";
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    let change = body.map(|Json(b)| b.change()).unwrap_or_default();
    let agent = p.agent().changed(change);
    engine_offers(&s, &t, ERROR, &agent).await?;
    let chats = s.chats.clone();
    let added = s
        .writer
        .call(move |c| {
            let removing = chats.being_removed();
            gathr::proposals::add_agent(c, &message, &agent, &removing)
        })
        .await;
    if let Some(Err(gathr::Error::Refused(why))) = &added {
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    let added = crate::team::written(&t, added, ERROR)?
        .map_err(|why| fail(status(&why), ERROR, why.why(), &t.0))?;
    record(&s, &t, "add", &added.agent, None);
    s.chats
        .publish(json!({ "type": "message", "chat": added.proposed_in, "message": added.block }));
    Ok(Json(
        json!({ "agent": added.agent, "message": added.block }),
    ))
}

/// The status a refused save answers with.
fn status(why: &Refused) -> StatusCode {
    match why {
        Refused::Taken | Refused::Removing | Refused::Moved => StatusCode::CONFLICT,
        Refused::Unknown => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    }
}

/// Refused unless the account's engine lists the agent's model with its effort (none set, nothing to check).
async fn engine_offers(
    s: &State,
    t: &Trace,
    error: &'static str,
    agent: &Agent,
) -> Result<(), Fail> {
    if agent.model.is_none() && agent.effort.is_none() {
        return Ok(());
    }
    let refused = |why: &str| Err(fail(StatusCode::BAD_REQUEST, error, why, &t.0));
    let Ok(account) = find_account(s, t, &agent.account, error).await else {
        return refused(Refused::NoAccount.why());
    };
    let offered = models::list(s, &t.0, &account)
        .await
        .map_err(|why| format!("Models weren't listed: {why}"))
        .and_then(|list| models::offered(&list, agent.model.as_deref(), agent.effort.as_deref()));
    offered.or_else(|why| refused(&why))
}

/// Checks the model and effort against the account's engine, then writes the agent: a new one, or
/// `edit`'s change and who made it, applied to the agent as saved at the write.
async fn save(
    s: &Arc<State>,
    t: &Trace,
    (change, error): (&'static str, &'static str),
    agent: Agent,
    edit: Option<(Change, Option<String>)>,
) -> Answer {
    let by = edit.as_ref().and_then(|(_, by)| by.clone());
    let refused = |status, why: &str| Err(fail(status, error, why, &t.0));
    engine_offers(s, t, error, &agent).await?;
    let chats = s.chats.clone();
    let saved = s
        .writer
        .call(move |conn| {
            // Read in the write's own call, so a removal that began first is always seen.
            let removing = chats.being_removed();
            match edit {
                Some((change, by)) => agents::change(conn, &agent, change, by.is_some(), &removing),
                None => agents::add(conn, &agent, &removing),
            }
        })
        .await;
    let agent = match saved {
        Some(Ok(Ok(agent))) => agent,
        Some(Ok(Err(why))) => return refused(status(&why), why.why()),
        Some(Err(e)) => return refused(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        None => {
            return refused(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the database didn't answer",
            );
        }
    };
    record(s, t, change, &agent, by);
    Ok(Json(json!(agent)))
}

/// Records the change and tells every window.
fn record(s: &State, t: &Trace, change: &'static str, agent: &Agent, by: Option<String>) {
    let event = Event::point("gathr.agent.change")
        .trace(&t.0)
        .ok()
        .attr("agent", agent.id.clone())
        .attr("account", agent.account.clone())
        .attr("model", agent.model.clone())
        .attr("effort", agent.effort.clone())
        .attr("approvals", agent.approvals.as_str())
        .attr("change", change)
        .attr("by", by);
    s.writer.record(event);
    s.chats.publish(json!({ "type": "agents" }));
}
