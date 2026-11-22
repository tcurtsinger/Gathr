//! Settings > General: `/api/settings`, as the person reads and changes it.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use gathr::event::Event;
use gathr::{assistant, folders};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/settings", get(settings).patch(change_settings))
        .route("/api/settings/background", get(crate::naming::listed))
}

/// The background model the person picked.
#[derive(Deserialize)]
struct Picked {
    account: String,
    model: String,
}

/// Settings > General: the agent folder (0034), the hand-off limit, Auto-compact, the person's name
/// and their assistant.
async fn settings(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let data = s.data.path().to_path_buf();
    db(&s, &t, "Settings failed", move |c| shown(c, &data)).await
}

#[derive(Deserialize, Default)]
struct Change {
    agent_folder: Option<String>,
    /// Set to a number, or `null` for no limit; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    handoff_limit: Option<Option<i64>>,
    /// Auto-compact (K3): one of 200000 to 500000, or `null` for each engine's own; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    auto_compact: Option<Option<i64>>,
    /// What agents call the person, or `null` for nothing; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    person_name: Option<Option<String>>,
    /// The assistant's agent id, never `null`; left out, unchanged.
    #[serde(default, deserialize_with = "not_null")]
    assistant: Option<String>,
    /// The background model, or `null` for Gathr's own pick; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    background_model: Option<Option<Picked>>,
}

/// A field that, when there, has a value: `null` is refused as a wrong type.
fn not_null<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// A field that's there, `null` included, apart from one that's left out.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

async fn change_settings(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Change>,
) -> Answer {
    let trace = t.0.clone();
    let folder = body
        .agent_folder
        .map(|f| std::path::PathBuf::from(f.trim()));
    if let Some(folder) = folder.clone() {
        let data = s.data.path().to_path_buf();
        let checked = tokio::task::spawn_blocking(move || folders::usable(&folder, &data)).await;
        let refused = checked.unwrap_or_else(|e| Err(format!("the folder wasn't checked: {e}")));
        if let Err(why) = refused {
            let event = Event::point("gathr.settings.change").trace(&trace);
            s.writer
                .record(event.error("refused").attr("why", why.clone()));
            return Err(fail(StatusCode::BAD_REQUEST, "Save failed", &why, &trace));
        }
    }
    let data = s.data.path().to_path_buf();
    db(&s, &t, "Save failed", move |c| {
        let mut saved = Ok(());
        if let Some(limit) = body.handoff_limit {
            saved = gathr::handoffs::set_limit(c, limit);
        }
        if let (Ok(()), Some(at)) = (&saved, body.auto_compact) {
            saved = gathr::context::set_auto_compact(c, at);
        }
        if let (Ok(()), Some(name)) = (&saved, &body.person_name) {
            saved = assistant::set_person_name(c, name.as_deref());
        }
        if let (Ok(()), Some(agent)) = (&saved, &body.assistant) {
            saved = assistant::set_assistant(c, agent);
        }
        if let (Ok(()), Some(picked)) = (&saved, &body.background_model) {
            let picked = picked
                .as_ref()
                .map(|p| (p.account.as_str(), p.model.as_str()));
            saved = gathr::titles::set_model(c, picked);
        }
        if let (Ok(()), Some(folder)) = (&saved, &folder) {
            folders::set_root(c, folder)?;
        }
        let event = Event::point("gathr.settings.change").trace(&trace);
        let event = match &saved {
            Ok(()) => event.ok(),
            Err(e) => event.error("refused").attr("why", e.to_string()),
        };
        event
            .attr("handoff_limit", gathr::handoffs::limit(c)?)
            .attr("auto_compact", gathr::context::auto_compact(c)?)
            .attr("assistant", assistant::assistant(c)?)
            .attr(
                "agent_folder",
                folders::root(c, &data)?.display().to_string(),
            )
            .insert(c)?;
        saved?;
        shown(c, &data)
    })
    .await
}

fn shown(
    c: &rusqlite::Connection,
    data: &std::path::Path,
) -> Result<serde_json::Value, gathr::Error> {
    let folder = folders::root(c, data)?.display().to_string();
    let handoff_limit = gathr::handoffs::limit(c)?;
    let auto_compact = gathr::context::auto_compact(c)?;
    Ok(json!({
        "agent_folder": folder,
        "handoff_limit": handoff_limit,
        "auto_compact": auto_compact,
        "person_name": assistant::person_name(c)?,
        "assistant": assistant::assistant(c)?,
        "background_model": gathr::titles::model(c)?
            .map(|(account, model)| json!({ "account": account, "model": model })),
    }))
}
