//! Delete a project or an agent (0069), and what each delete would do, for the confirm's words.
//! A project goes with its chats (each through a chat's own delete), then its rows, then its own
//! folders by the sweep. An agent's work stops the gentle way, it is removed in one transaction,
//! then what it was the only agent of goes. Sent again after a failure, either carries on.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::deleting::{self, Plan};
use gathr::event::Event;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, fail};
use crate::team::written;

const ERROR: &str = "Delete failed";
const COUNTS: &str = "Counts failed";

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/projects/{project}/delete", get(project_counts))
        .route("/api/agents/{agent}/delete", get(agent_counts))
}

async fn project_counts(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    crate::projects::find(&s, &t, &id, COUNTS).await?;
    let planned = s.writer.call(move |c| deleting::project_plan(c, &id));
    Ok(Json(json!(written(&t, planned.await, COUNTS)?.counts())))
}

async fn agent_counts(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    crate::agents::find(&s, &t, &id, COUNTS).await?;
    let planned = s.writer.call(move |c| deleting::agent_plan(c, &id));
    Ok(Json(json!(written(&t, planned.await, COUNTS)?.counts())))
}

pub async fn project(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    crate::projects::find(&s, &t, &id, ERROR).await?;
    let key = id.clone();
    let planned = s.writer.call(move |c| deleting::project_plan(c, &key));
    let plan = written(&t, planned.await, ERROR)?;
    let mut left = Vec::new();
    for chat in &plan.chats {
        left.extend(crate::archive::delete(&s, &t, chat).await?);
    }
    left.extend(finish_project(&s, &t, &id).await?);
    Ok(answer(&plan, left))
}

/// The project's rows, once its chats are gone, then its own folders: what couldn't be removed.
async fn finish_project(s: &Arc<State>, t: &Trace, id: &str) -> Result<Vec<String>, Fail> {
    let (key, data, trace) = (id.to_string(), s.data.path().to_path_buf(), t.0.clone());
    let deleted = s
        .writer
        .call(move |c| deleting::delete_project(c, &key, &data, &trace));
    let parts = written(t, deleted.await, ERROR)?;
    let left = crate::cleanup::sweep_paths(&s.chats, parts).await;
    let event = Event::point("gathr.project.files")
        .trace(&t.0)
        .attr("gathr.project.id", id);
    s.writer.record(match left.is_empty() {
        true => event.ok(),
        false => event.error("left").attr("why", left.join("; ")),
    });
    s.chats
        .publish(json!({ "type": "project", "project": id, "change": "deleted" }));
    Ok(left)
}

pub async fn agent(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    // A removed agent's delete carries on with what it was still the only agent of.
    let key = id.clone();
    let named = s.writer.call(move |c| gathr::agents::name(c, &key));
    if written(&t, named.await, ERROR)?.is_none() {
        return Err(fail(StatusCode::NOT_FOUND, ERROR, "no such agent", &t.0));
    }
    s.chats.retire(&id);
    let (key, trace) = (id.clone(), t.0.clone());
    let removed = s
        .writer
        .call(move |c| deleting::remove_agent(c, &key, &trace));
    let removed = written(&t, removed.await, ERROR)?;
    s.chats.start(removed.starts);
    for (chat, members) in removed.chats {
        crate::team::changed(&s, &t, &chat, members);
    }
    for project in &removed.projects {
        s.chats
            .publish(json!({ "type": "project", "project": project, "change": "changed" }));
    }
    s.chats.pauses_changed().await;
    let plan = removed.plan;
    let mut left = Vec::new();
    let deleted = async {
        for chat in &plan.chats {
            left.extend(crate::archive::delete(&s, &t, chat).await?);
        }
        for project in &plan.projects {
            left.extend(finish_project(&s, &t, project).await?);
        }
        Ok::<(), Fail>(())
    }
    .await;
    let counts = plan.counts();
    let event = Event::point("gathr.agent.change")
        .trace(&t.0)
        .attr("agent", id.clone())
        .attr("change", "remove")
        .attr("gathr.task.cancelled", counts.tasks)
        .attr("chats", counts.chats)
        .attr("projects", counts.projects)
        .attr("leads", counts.leads.clone());
    s.writer.record(match (&deleted, left.is_empty()) {
        (Err(failed), _) => event.error("delete").attr("why", failed.why().to_string()),
        (Ok(()), false) => event.error("left").attr("why", left.join("; ")),
        (Ok(()), true) => event.ok(),
    });
    s.chats.publish(json!({ "type": "agents" }));
    deleted?;
    Ok(answer(&plan, left))
}

/// A delete's answer: what it did, in the confirm's counts, and what it left.
fn answer(plan: &Plan, left: Vec<String>) -> Json<Value> {
    let mut answer = json!(plan.counts());
    answer["deleted"] = true.into();
    answer["left"] = json!(left);
    Json(answer)
}
