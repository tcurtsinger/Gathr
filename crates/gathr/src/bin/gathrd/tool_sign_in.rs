//! Signing a tool in through the browser (`gathr::tool_sign_in`). Sign in opens the tool's sign-in
//! page in the person's browser; the page comes back to `BACK` on this service, admitted only by the
//! `state` a waiting sign-in made, once. Waiting sign-ins are kept in memory only: they end on
//! Cancel, when they finish, and when the service stops. The end is sent live as `tools`, with the
//! sign-in it ends and the tool, or why it didn't work.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use gathr::event::Event;
use gathr::ids::{self, now_us, ulid};
use gathr::tool_sign_in::{self, Refused, Started};
use gathr::web_tools;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, fail};
use crate::tools::relay;

/// Where the browser comes back to, on 127.0.0.1 and the service's port.
const BACK: &str = "/tools/signed-in";

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/tools/sign-in", post(start))
        .route("/api/tools/sign-in/{state}", delete(cancel))
        .route(BACK, get(back))
}

struct Waiting {
    started: Started,
    link: String,
    /// The name given, if any; a new tool is otherwise named by its own server info.
    name: Option<String>,
    /// The name its sentences use: the name given or the tool's, else the link's host.
    called: String,
    /// The tool signing in again; none for a new one.
    tool: Option<String>,
    /// The Client ID it signs in with, if any: kept with the tool once it's signed in.
    client_id: Option<String>,
}

/// The sign-ins waiting for the browser, by their `state`.
static WAITING: Mutex<Option<HashMap<String, Waiting>>> = Mutex::new(None);

fn waiting() -> std::sync::MutexGuard<'static, Option<HashMap<String, Waiting>>> {
    WAITING.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Deserialize)]
struct Start {
    link: Option<String>,
    name: Option<String>,
    /// A tool already added, to sign in again.
    tool: Option<String>,
    /// The Client ID an admin made for Gathr, and its secret: given for a new tool, or to change a
    /// tool's. A tool signing in again without them uses its own.
    client_id: Option<String>,
    client_secret: Option<String>,
}

/// The address the browser comes back to, which an admin gives the app they make for Gathr.
pub fn redirect(s: &State) -> String {
    format!("http://127.0.0.1:{}{BACK}", s.port)
}

/// Starts a sign-in for a new tool at `link`, or for `tool` again, and opens its page in the
/// browser. Answers the page, for Open sign-in page, and the sign-in's `state`, for Cancel; or, when
/// the tool needs a Client ID and has none, `registers: false` and the address to give its admin.
async fn start(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(asked): Json<Start>,
) -> Answer {
    let refused = |why: &str| Err(fail(StatusCode::CONFLICT, "Sign-in failed", why, &t.0));
    let given = |text: Option<String>| text.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let name = given(asked.name);
    let mut client = given(asked.client_id).map(|id| tool_sign_in::Client {
        id,
        secret: given(asked.client_secret),
    });
    let (link, called) = match &asked.tool {
        Some(id) => {
            let (id, writer) = (id.clone(), s.writer.clone());
            let read = s.writer.call(move |conn| {
                let tool = web_tools::get(conn, &id)?;
                let secret = match web_tools::access(conn, &id, &writer) {
                    Ok(web_tools::Access::SignIn(sign_in)) => sign_in.client_secret(),
                    _ => None,
                };
                Ok::<_, gathr::Error>((tool, secret))
            });
            match read.await {
                Some(Ok((tool, secret))) => {
                    if client.is_none() {
                        client = tool.client_id.map(|id| tool_sign_in::Client { id, secret });
                    }
                    (tool.link, tool.name)
                }
                Some(Err(e)) => return refused(&e.to_string()),
                None => return refused("the database didn't answer"),
            }
        }
        None => {
            let link = asked.link.unwrap_or_default().trim().to_string();
            match web_tools::checked(&link) {
                Ok(host) => (link, name.clone().unwrap_or(host)),
                Err(e) => return refused(&e.to_string()),
            }
        }
    };
    let client_id = client.as_ref().map(|c| c.id.clone());
    let started = match tool_sign_in::start(&link, &redirect(&s), client).await {
        Ok(Some(started)) => started,
        Ok(None) => {
            return Ok(Json(
                json!({ "registers": false, "redirect": redirect(&s) }),
            ));
        }
        Err(e) => {
            record(&s, &t, &e).await;
            return refused(&e.said(&called));
        }
    };
    // The window's Open sign-in page opens it again when this didn't.
    let _ = gathr::platform::open_browser(&started.url);
    let answer = json!({ "sign_in": started.state, "url": started.url });
    let state = started.state.clone();
    let sign_in = Waiting {
        started,
        link,
        name,
        called,
        tool: asked.tool,
        client_id,
    };
    waiting()
        .get_or_insert_with(HashMap::new)
        .insert(state, sign_in);
    Ok(Json(answer))
}

/// Cancel: the sign-in ends, and its page coming back later finds nothing.
async fn cancel(Path(state): Path<String>) -> Answer {
    if let Some(all) = waiting().as_mut() {
        all.remove(&state);
    }
    Ok(Json(json!({ "cancelled": true })))
}

/// The sign-in's failure, for its reference: why in plain words, and the library's.
async fn record(s: &State, t: &Trace, refused: &Refused) {
    let event = Event::point("gathr.tools.sign_in")
        .trace(&t.0)
        .error("sign_in")
        .attr("reason", refused.detail());
    s.writer.record_now(event).await;
}

/// Where the browser comes back: with `code` when the person signed in, `error` when not. A
/// `state` no sign-in is waiting with is refused, so each one comes back once.
async fn back(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Query(query): Query<HashMap<String, String>>,
) -> (StatusCode, Html<String>) {
    let state = query.get("state").cloned().unwrap_or_default();
    let found = waiting().as_mut().and_then(|all| all.remove(&state));
    let Some(sign_in) = found else {
        let words = "This sign-in isn’t waiting anymore. Go back to Gathr to sign in again.";
        return (StatusCode::BAD_REQUEST, page("Not signed in", words));
    };
    let called = sign_in.called.clone();
    match finish(&s, sign_in, &query).await {
        Ok((id, called)) => {
            let _ = s
                .chats
                .live
                .send(json!({ "type": "tools", "sign_in": state, "tool": id }));
            let words = format!("Signed in to {called}. You can go back to Gathr.");
            (StatusCode::OK, page("Signed in", &words))
        }
        Err(refused) => {
            record(&s, &t, &refused).await;
            let why = refused.said(&called);
            let reference = ids::reference(&t.0);
            let told = json!({ "type": "tools", "sign_in": state, "why": why, "ref": reference });
            let _ = s.chats.live.send(told);
            let words = format!("{why} Go back to Gathr to try again.");
            (StatusCode::OK, page("Sign-in failed", &words))
        }
    }
}

/// Swaps the code for the sign-in and keeps it sealed: a new tool is added, named as one added with
/// a key is; a tool signing in again keeps its name. Answers the tool's id and name.
async fn finish(
    s: &State,
    sign_in: Waiting,
    query: &HashMap<String, String>,
) -> Result<(String, String), Refused> {
    if let Some(error) = query.get("error") {
        return Err(match error.as_str() {
            "access_denied" => Refused::Denied,
            other => Refused::Failed(format!("the sign-in page answered {other}")),
        });
    }
    let code = query
        .get("code")
        .ok_or_else(|| Refused::Failed("no code came back".into()))?;
    let issuer = query.get("iss").map(String::as_str);
    let (manager, text) = tool_sign_in::finish(sign_in.started, code, issuer).await?;
    let (id, name) = match sign_in.tool.clone() {
        Some(id) => (id, sign_in.called),
        None => {
            let link = &sign_in.link;
            let remote = web_tools::connect_signed_in(link, manager, web_tools::CONNECT).await;
            let own = remote.ok().as_ref().and_then(web_tools::own_name);
            (
                ulid(now_us()),
                sign_in.name.or(own).unwrap_or(sign_in.called),
            )
        }
    };
    let stored = |e: gathr::Error| Refused::Failed(e.to_string());
    let sealed = tool_sign_in::sealed(&id, &text).await.map_err(stored)?;
    let (kept, named, link, again) = (id.clone(), name.clone(), sign_in.link, sign_in.tool);
    let client_id = sign_in.client_id;
    let saved = s
        .writer
        .call(move |conn| {
            let client_id = client_id.as_deref();
            match again {
                Some(_) => {
                    web_tools::set_sign_in(conn, &kept, &sealed)?;
                    web_tools::set_client_id(conn, &kept, client_id)
                }
                None => web_tools::add_signed_in(conn, &kept, &named, &link, &sealed, client_id)
                    .map(|_| ()),
            }
        })
        .await;
    saved
        .unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())))
        .map_err(stored)?;
    tool_sign_in::forget(&id);
    relay::forget(&id);
    Ok((id, name))
}

/// The small page the browser ends on.
fn page(title: &str, words: &str) -> Html<String> {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let (title, words) = (escape(title), escape(words));
    Html(format!(
        "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><title>Gathr</title><style>:root{{color-scheme:light dark}}\
         body{{font:15px system-ui,sans-serif;max-width:32rem;margin:20vh auto;padding:0 16px}}\
         h1{{font-size:20px}}</style><h1>{title}</h1><p>{words}</p>"
    ))
}
