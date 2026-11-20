//! Settings > General: `/api/settings`, as the person reads and changes it, and as an agent does
//! through its settings tool.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use gathr::event::Event;
use gathr::folders;
use gathr::tasks::stale;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/settings", get(settings).patch(change_settings))
}

/// Settings > General: the stale days (F11), after how many days without a move a task is stale and
/// cancelled, the agent folder (0034) and the hand-off limit (plans/agent-messages.md).
async fn settings(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    settings_shown(&s, &t).await
}

/// Settings > General as `GET /api/settings` answers it.
pub async fn settings_shown(s: &Arc<State>, t: &Trace) -> Answer {
    let data = s.data.path().to_path_buf();
    db(s, t, "Settings failed", move |c| shown(c, &data)).await
}

#[derive(Deserialize, Default)]
pub struct Change {
    pub stale_days: Option<i64>,
    pub cancel_days: Option<i64>,
    pub agent_folder: Option<String>,
    /// Set to a number, or `null` for no limit; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    pub handoff_limit: Option<Option<i64>>,
    /// Auto-compact (K3): one of 200000 to 500000, or `null` for each engine's own; left out, unchanged.
    #[serde(default, deserialize_with = "present")]
    pub auto_compact: Option<Option<i64>>,
}

/// A field that's there, `null` included, apart from one that's left out.
pub fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<i64>>, D::Error> {
    Option::<i64>::deserialize(d).map(Some)
}

async fn change_settings(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Change>,
) -> Answer {
    change_settings_as(&s, &t, body, None).await
}

/// A change by the person (`by` none) or by an agent through its settings tool (0043 C5a).
pub async fn change_settings_as(
    s: &Arc<State>,
    t: &Trace,
    body: Change,
    by: Option<String>,
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
            s.writer.record(
                event
                    .error("refused")
                    .attr("why", why.clone())
                    .attr("by", by),
            );
            return Err(fail(StatusCode::BAD_REQUEST, "Save failed", &why, &trace));
        }
    }
    let data = s.data.path().to_path_buf();
    db(s, t, "Save failed", move |c| {
        let was = stale::days(c)?;
        let days = stale::Days {
            stale: body.stale_days.unwrap_or(was.stale),
            cancel: body.cancel_days.unwrap_or(was.cancel),
        };
        let mut saved = stale::set_days(c, days);
        if let (Ok(()), Some(limit)) = (&saved, body.handoff_limit) {
            saved = gathr::handoffs::set_limit(c, limit);
        }
        if let (Ok(()), Some(at)) = (&saved, body.auto_compact) {
            saved = gathr::context::set_auto_compact(c, at);
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
            .attr("by", by)
            .attr("stale_days", days.stale)
            .attr("cancel_days", days.cancel)
            .attr("handoff_limit", gathr::handoffs::limit(c)?)
            .attr("auto_compact", gathr::context::auto_compact(c)?)
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
    let days = stale::days(c)?;
    let folder = folders::root(c, data)?.display().to_string();
    let handoff_limit = gathr::handoffs::limit(c)?;
    let auto_compact = gathr::context::auto_compact(c)?;
    Ok(
        json!({ "stale_days": days.stale, "cancel_days": days.cancel, "agent_folder": folder,
        "handoff_limit": handoff_limit, "auto_compact": auto_compact }),
    )
}
