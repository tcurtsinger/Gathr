//! The person's files for a message, and the tree, words and pictures the window shows from a
//! chat's folders (`gathr::files`).

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State as Extract};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::{changes, files};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(
            "/api/chats/{chat}/files",
            post(upload)
                .layer(DefaultBodyLimit::max(files::MOST))
                .get(tree),
        )
        .route("/api/chats/{chat}/file", get(text))
        .route("/api/chats/{chat}/picture", get(picture))
        .route("/api/chats/{chat}/changes", get(changes))
        .route("/api/chats/{chat}/change", get(change))
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

/// Saves the body as the file `name` in the chat's files folder: its saved name and path.
async fn upload(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Named>,
    body: Bytes,
) -> Answer {
    known(&s, &t, &chat).await?;
    let dir = folder(&s, &t, &chat, "Upload failed").await?;
    let saved = tokio::task::spawn_blocking(move || {
        let name = files::save(&dir, &q.name, &body)?;
        Ok::<_, gathr::Error>((dir.join(&name), name))
    })
    .await;
    match saved {
        Ok(Ok((path, name))) => Ok(Json(json!({ "name": name, "path": path }))),
        Ok(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Upload failed",
            &e.to_string(),
            &t.0,
        )),
        Err(e) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Upload failed",
            &e.to_string(),
            &t.0,
        )),
    }
}

/// The chat's files folder, recorded.
pub async fn folder(
    s: &State,
    t: &Trace,
    chat: &str,
    error: &'static str,
) -> Result<std::path::PathBuf, Fail> {
    let (id, data) = (chat.to_string(), s.data.path().to_path_buf());
    let Json(dir) = db(s, t, error, move |conn| {
        Ok(json!(files::folder(conn, &data, &id)?))
    })
    .await?;
    Ok(std::path::PathBuf::from(dir.as_str().unwrap_or_default()))
}

/// The lines naming `names`, the chat's saved files, refused when one isn't there.
pub async fn lines(
    s: &State,
    t: &Trace,
    chat: &str,
    names: &[String],
    error: &'static str,
) -> Result<String, Fail> {
    if names.is_empty() {
        return Ok(String::new());
    }
    let dir = folder(s, t, chat, error).await?;
    files::lines(&dir, names)
        .map_err(|e| fail(StatusCode::BAD_REQUEST, error, &e.to_string(), &t.0))
}

#[derive(Deserialize)]
pub struct Shown {
    pub path: String,
}

/// A picture from one of the chat's folders, for the window's tiles and viewer.
async fn picture(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Response, Fail> {
    known(&s, &t, &chat).await?;
    let (id, data) = (chat.clone(), s.data.path().to_path_buf());
    let Json(roots) = db(&s, &t, "Picture failed", move |conn| {
        Ok(json!(files::roots(conn, &data, &id)?))
    })
    .await?;
    let roots: Vec<std::path::PathBuf> = serde_json::from_value(roots).unwrap_or_default();
    let read = tokio::task::spawn_blocking(move || {
        let (path, kind) = files::picture(&roots, &q.path)?;
        Some((std::fs::read(path).ok()?, kind))
    })
    .await;
    let Ok(Some((bytes, kind))) = read else {
        let why = "it isn't a picture in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Picture failed", why, &t.0));
    };
    Ok((safe(kind), bytes).into_response())
}

/// A file's bytes go out as only what they say they are, never run, never kept.
pub fn safe(kind: &'static str) -> [(header::HeaderName, &'static str); 4] {
    [
        (header::CONTENT_TYPE, kind),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'",
        ),
        (header::CACHE_CONTROL, "no-store"),
    ]
}

/// The chat's tree as the window shows it: the folders of this chat only.
pub async fn tree_of(
    s: &State,
    t: &Trace,
    chat: &str,
    error: &'static str,
) -> Result<files::Tree, Fail> {
    known(s, t, chat).await?;
    let (id, data) = (chat.to_string(), s.data.path().to_path_buf());
    let Json(tree) = db(s, t, error, move |conn| {
        Ok(json!(files::tree(conn, &data, &id)?))
    })
    .await?;
    serde_json::from_value(tree).map_err(|e| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            &e.to_string(),
            &t.0,
        )
    })
}

#[derive(Deserialize)]
struct Folder {
    path: Option<String>,
    find: Option<String>,
}

/// Without a path, the tree's top folders; with one, that folder's folders and files; with `find`,
/// the files whose name holds its words.
async fn tree(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Folder>,
) -> Answer {
    let tree = tree_of(&s, &t, &chat, "Files failed").await?;
    if let Some(words) = q.find {
        return Ok(Json(json!({ "found": files::find(&tree, &words).await })));
    }
    let Some(path) = q.path else {
        return Ok(Json(json!(tree)));
    };
    let listed = tokio::task::spawn_blocking(move || files::listing(&tree.roots(), &path)).await;
    let Ok(Some(entries)) = listed else {
        let why = "it isn't a folder in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Files failed", why, &t.0));
    };
    Ok(Json(json!({ "entries": entries })))
}

/// A file's words, for its tab in the window: 404 when it isn't a file in this chat's folders,
/// 413 over `files::SHOWN`, 415 when it isn't text.
async fn text(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Response, Fail> {
    let tree = tree_of(&s, &t, &chat, "File failed").await?;
    let read = tokio::task::spawn_blocking(move || files::text(&tree.roots(), &q.path)).await;
    let (status, why) = match read {
        Ok(Ok(words)) => return Ok((safe("text/plain; charset=utf-8"), words).into_response()),
        Ok(Err(files::Unshown::Gone)) | Err(_) => (
            StatusCode::NOT_FOUND,
            "it isn't a file in this chat's folders",
        ),
        Ok(Err(files::Unshown::Large)) => (StatusCode::PAYLOAD_TOO_LARGE, "it's too large to show"),
        Ok(Err(files::Unshown::NotText)) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "it isn't text"),
    };
    Err(fail(status, "File failed", why, &t.0))
}

/// The chat's tree, its members as (id, name), and the checkouts outside its worktrees its agents
/// wrote in (`changes::outside`).
async fn wrote(s: &State, t: &Trace, chat: &str, error: &'static str) -> Result<Written, Fail> {
    let tree = tree_of(s, t, chat, error).await?;
    let id = chat.to_string();
    let Json(found) = db(s, t, error, move |conn| {
        let members = gathr::team::members(conn, &id)?;
        let members: Vec<_> = members.into_iter().map(|m| (m.agent, m.name)).collect();
        Ok(json!({ "members": members, "written": changes::written(conn, &id)? }))
    })
    .await?;
    let members = serde_json::from_value(found["members"].clone()).unwrap_or_default();
    let steps: Vec<(String, String)> =
        serde_json::from_value(found["written"].clone()).unwrap_or_default();
    let own: Vec<_> = tree.agents.iter().map(|p| p.path.clone()).collect();
    let data = s.data.path().to_path_buf();
    let outside = match steps.is_empty() {
        true => Default::default(),
        false => tokio::task::spawn_blocking(move || changes::outside(&steps, &own, &data))
            .await
            .unwrap_or_default(),
    };
    Ok(Written {
        tree,
        members,
        outside,
    })
}

struct Written {
    tree: files::Tree,
    members: Vec<(String, String)>,
    outside: std::collections::BTreeMap<(String, std::path::PathBuf), Vec<String>>,
}

/// What each agent changed in the chat: per agent, its worktree's changed files with its branch,
/// and per other git checkout it wrote in with its file tools, what isn't committed of those files;
/// at most `changes::MOST_FILES` in all. Every worktree and checkout is read at once; a chat with
/// neither runs no git.
async fn changes(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    let Written {
        tree,
        members,
        outside,
    } = wrote(&s, &t, &chat, "Changes failed").await?;
    let mut reads = tokio::task::JoinSet::new();
    if let Some(project) = tree.project {
        for part in tree
            .agents
            .into_iter()
            .filter(|p| p.path.join(".git").exists())
        {
            let (project, agent) = (project.path.clone(), part.agent.unwrap_or_default());
            reads.spawn(async move {
                let change = changes::list(&project, &part.path, changes::MOST_FILES).await;
                (agent, None, change)
            });
        }
    }
    for ((agent, top), paths) in outside {
        reads.spawn(async move {
            let change = changes::uncommitted(&top, &paths, changes::MOST_FILES).await;
            (agent, Some(top), change)
        });
    }
    let (mut own, mut elsewhere) = (std::collections::HashMap::new(), Vec::new());
    while let Some(read) = reads.join_next().await {
        let Ok((agent, top, change)) = read else {
            continue;
        };
        match top {
            None => _ = own.insert(agent, change.map_err(|e| failed("Changes failed", e, &t))?),
            // A checkout git can't read (one with no commit yet) is left out.
            Some(top) => elsewhere.extend(change.ok().map(|c| (agent, top, c))),
        }
    }
    elsewhere.sort_by(|a, b| a.1.cmp(&b.1));
    let (mut room, mut more) = (changes::MOST_FILES, false);
    let mut take = |change: changes::Change| {
        let mut files = change.files;
        more |= change.more || files.len() > room;
        files.truncate(room);
        room -= files.len();
        files
    };
    let mut agents = Vec::new();
    for (id, name) in members {
        let worktree = own.remove(&id);
        let branch = worktree.as_ref().map(|w| w.branch.clone());
        let files = worktree.map(&mut take);
        let checkouts: Vec<_> = elsewhere
            .extract_if(.., |(agent, _, _)| *agent == id)
            .filter_map(|(_, folder, change)| {
                let files = take(change);
                (!files.is_empty()).then(|| json!({ "folder": folder, "files": files }))
            })
            .collect();
        if files.is_none() && checkouts.is_empty() {
            continue;
        }
        let files = files.unwrap_or_default();
        agents.push(
            json!({ "id": id, "name": name, "branch": branch, "files": files,
            "outside": checkouts }),
        );
    }
    Ok(Json(json!({ "agents": agents, "more": more })))
}

#[derive(Deserialize)]
struct Changed {
    agent: String,
    path: String,
    from: Option<String>,
    /// The checkout outside the chat's worktrees the file is in, as the list named it.
    folder: Option<String>,
}

/// One file's change in an agent's worktree, or in a checkout it wrote in, as lines to draw.
async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Changed>,
) -> Answer {
    if let Some(folder) = q.folder {
        let Written { outside, .. } = wrote(&s, &t, &chat, "Change failed").await?;
        let key = (q.agent, std::path::PathBuf::from(folder));
        // Only a file the agent wrote there is read.
        if !outside
            .get(&key)
            .is_some_and(|paths| paths.contains(&q.path))
        {
            let why = "that agent didn't write that file in this chat";
            return Err(fail(StatusCode::NOT_FOUND, "Change failed", why, &t.0));
        }
        let lines = changes::uncommitted_file(&key.1, &q.path).await;
        let lines = lines.map_err(|e| failed("Change failed", e, &t))?;
        return Ok(Json(json!(lines)));
    }
    let tree = tree_of(&s, &t, &chat, "Change failed").await?;
    let worktree = tree
        .agents
        .into_iter()
        .find(|p| p.agent.as_deref() == Some(&q.agent) && p.path.join(".git").exists());
    let (Some(project), Some(worktree)) = (tree.project, worktree) else {
        let why = "that agent has no worktree in this chat";
        return Err(fail(StatusCode::NOT_FOUND, "Change failed", why, &t.0));
    };
    let lines = changes::file(&project.path, &worktree.path, &q.path, q.from.as_deref()).await;
    let lines = lines.map_err(|e| failed("Change failed", e, &t))?;
    Ok(Json(json!(lines)))
}

fn failed(error: &'static str, e: gathr::Error, t: &Trace) -> Fail {
    let status = match e {
        gathr::Error::Refused(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    fail(status, error, &e.to_string(), &t.0)
}
