//! Gathr tells the person outside the window (0058): every few seconds it reads which chats need
//! them and whether the whole app is paused, and shows a system notification for what
//! `gathr::notice` decides. A click brings the window forward on that chat.
//!
//! No test seam: the thread, the window's focus and the plugin are glue around `notice::decide`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gathr::http;
use gathr::notice::{self, Notice, Start};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_notification::{ActionPerformed, NotificationExt};

use crate::{Service, reopen, report, update};

/// The notifications kept for a click. One is reported only while it's on screen, so an older one
/// won't be clicked.
const KEPT: usize = 50;

/// Each notification kept, by its id, with the chat a click on it opens.
type Shown = Arc<Mutex<BTreeMap<i32, Option<String>>>>;

/// Reads the service every few seconds, from `setup`. `after_update`: this start follows the
/// window's own update, so what waits now was already told.
pub fn run(app: &AppHandle, after_update: bool) {
    let shown = Shown::default();
    let (on_click, kept) = (app.clone(), shown.clone());
    // Before any notification: the plugin reports a click only on one shown while a handler exists.
    let _ = app
        .notification()
        .on_action(move |performed| clicked(&on_click, &kept, performed));
    let start = match after_update {
        true => Start::AfterUpdate,
        false => Start::Other,
    };
    // A visit shorter than a tick still counts: the window notes each focus, a notification's
    // click included (`reopen` focuses it), and the next read takes the note.
    let visited = Arc::new(AtomicBool::new(false));
    if let Some(window) = app.get_webview_window("main") {
        let noted = visited.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::Focused(true) = event {
                noted.store(true, Ordering::SeqCst);
            }
        });
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let (mut state, mut next) = (None, 1);
        loop {
            std::thread::sleep(notice::EVERY);
            let now = tick(&app, state.as_ref(), start, &visited, &shown, &mut next);
            if let Some(now) = now {
                state = Some(now);
            }
        }
    });
}

/// One read, and what it decides shown and recorded. None when there was no service to read, or
/// it didn't answer both routes: the last state stays, and so does a visit's note.
fn tick(
    app: &AppHandle,
    last: Option<&notice::State>,
    start: Start,
    visited: &AtomicBool,
    shown: &Shown,
    next: &mut i32,
) -> Option<notice::State> {
    let since = Instant::now();
    let info = app.state::<Service>().info.lock().unwrap().clone()?;
    let updates = app.try_state::<update::Updates>();
    if updates.is_some_and(|u| u.installing.load(Ordering::SeqCst)) {
        return None;
    }
    let get = |path: &str| {
        let answer = http::call(info.port, &info.token, "GET", path).ok();
        answer.filter(|a| a.status == 200).map(|a| a.json())
    };
    let now = notice::read(get("/api/chats").as_ref(), get("/api/pauses").as_ref())?;
    let read = |path: String| get(&path).unwrap_or(Value::Null);
    let (state, notices) = notice::decide(
        last,
        start,
        &now,
        front(app),
        visited.swap(false, Ordering::SeqCst),
        |chat| read(format!("/api/chats/{chat}/waiting")),
        |chat| read(format!("/api/chats/{chat}")),
    );
    for notice in notices {
        tell(app, notice, shown, next, since);
    }
    Some(state)
}

/// Whether the window is in front of the person: shown, not minimized, and focused.
fn front(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    window.is_visible().unwrap_or(false)
        && !window.is_minimized().unwrap_or(true)
        && window.is_focused().unwrap_or(false)
}

/// Shows what was decided, and records it as `gathr.ui.notify`.
fn tell(app: &AppHandle, notice: Notice, shown: &Shown, next: &mut i32, since: Instant) {
    let (mut attrs, failed) = match notice {
        Notice::Show {
            chat,
            title,
            body,
            why,
            rose,
        } => {
            let (id, failed) = show(app, shown, next, &title, &body, Some(chat.clone()));
            let mut attrs = json!({ "outcome": "shown", "chat": chat, "id": id, "rose": rose });
            if let Some(why) = why {
                attrs["why"] = why.into();
            }
            (attrs, failed)
        }
        Notice::Summary { at, count, chat } => {
            let title = match at {
                "resume" => "Resumed",
                _ => "Waiting on you",
            };
            let body = notice::summary_body(count);
            let (id, failed) = show(app, shown, next, title, &body, chat.clone());
            let attrs =
                json!({ "outcome": "shown", "summary": count, "at": at, "id": id, "chat": chat });
            (attrs, failed)
        }
        Notice::Held { chat } => (
            json!({ "outcome": "held", "chat": chat, "why": "pause" }),
            None,
        ),
        Notice::Skipped { chat, why } => (
            json!({ "outcome": "skipped", "chat": chat, "why": why }),
            None,
        ),
    };
    if let Some(message) = &failed {
        attrs["message"] = message.as_str().into();
    }
    report(
        app,
        "gathr.ui.notify",
        since,
        &attrs,
        failed.map(|_| "show".into()),
    );
}

/// Shows one notification, kept for its click: its id, and why it couldn't be shown.
fn show(
    app: &AppHandle,
    shown: &Shown,
    next: &mut i32,
    title: &str,
    body: &str,
    chat: Option<String>,
) -> (i32, Option<String>) {
    let id = *next;
    *next += 1;
    {
        let mut kept = shown.lock().unwrap();
        kept.insert(id, chat);
        while kept.len() > KEPT {
            kept.pop_first();
        }
    }
    let builder = app.notification().builder().id(id).title(title).body(body);
    (id, builder.show().err().map(|e| e.to_string()))
}

/// A click brings the window forward, as the tray's Open does, on the notification's chat: with
/// none for a summary, or for a notification no longer kept.
fn clicked(app: &AppHandle, shown: &Shown, performed: &ActionPerformed) {
    let Some(id) = performed.notification().map(|n| n.id()) else {
        return;
    };
    let chat = shown.lock().unwrap().remove(&id).flatten();
    let attrs = json!({ "outcome": "clicked", "id": id, "chat": chat });
    report(app, "gathr.ui.notify", Instant::now(), &attrs, None);
    reopen(app, "notification", chat);
}
