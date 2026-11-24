//! Save, from the card of a change the assistant proposed to an agent or a project (`propose_agent_change`,
//! `propose_project_change`): saved as the person's own change on the agent's page or in the project's settings, and
//! the proposal marked saved; once only.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use gathr::proposals::{self, AgentChange, ProjectChange};
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, fail};
use crate::team::{Seat, written};

const ERROR: &str = "Save failed";

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/proposals/{message}/save", post(save))
}

enum Proposed {
    Agent(AgentChange),
    Project(ProjectChange),
}

async fn save(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(message): Path<String>,
) -> Answer {
    let id = message.clone();
    let found = s
        .writer
        .call(move |c| {
            if let Some(change) = proposals::find_agent_change(c, &id)? {
                return Ok(Some(Proposed::Agent(change)));
            }
            Ok(proposals::find_project_change(c, &id)?.map(Proposed::Project))
        })
        .await;
    let Some(proposed) = written(&t, found, ERROR)? else {
        return Err(fail(StatusCode::NOT_FOUND, ERROR, "no such proposal", &t.0));
    };
    let body = match proposed {
        Proposed::Agent(mut p) => {
            refuse_saved(&t, p.saved)?;
            let _ = crate::agents::change_as(&s, &t, &p.agent, p.change(), None).await?;
            p.saved = true;
            p.body()
        }
        Proposed::Project(mut p) => {
            refuse_saved(&t, p.saved)?;
            let (name, folder, team) = p.change();
            let members = team.map(|team| {
                let seats = team.into_iter();
                seats
                    .map(|(agent, role)| Seat {
                        agent,
                        role: role.as_str().to_string(),
                    })
                    .collect()
            });
            let settings = crate::projects::Settings {
                name,
                folder: folder.map(PathBuf::from),
                members,
            };
            let _ = crate::projects::change_to(&s, &t, p.project.clone(), settings).await?;
            p.saved = true;
            p.body()
        }
    };
    let marked = s
        .writer
        .call(move |c| proposals::mark_saved(c, &message, &body))
        .await;
    let (block, chat): (Value, String) = written(&t, marked, ERROR)?;
    s.chats
        .publish(json!({ "type": "message", "chat": chat, "message": block }));
    Ok(Json(json!({ "message": block })))
}

fn refuse_saved(t: &Trace, saved: bool) -> Result<(), crate::routes::Fail> {
    if saved {
        let why = "This change was saved already.";
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    Ok(())
}
