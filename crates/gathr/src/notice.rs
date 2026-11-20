//! When the window tells the person outside it (0058): from the chat list's `needs`, whether a chat
//! started needing them since the window was last in front. The app's thread reads the routes and
//! shows what this decides.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use crate::approvals::GATHR_ASKS;

/// How often the window reads the service.
pub const EVERY: Duration = Duration::from_secs(5);

/// The chats that need the person, each with its title, and whether the whole app is paused.
#[derive(Debug)]
pub struct Read {
    pub needs: BTreeMap<String, String>,
    pub paused: bool,
}

/// A read of `GET /api/chats` and `GET /api/pauses`: none unless both answered, so a pauses read
/// that failed during a pause never looks like a resume.
pub fn read(chats: Option<&Value>, pauses: Option<&Value>) -> Option<Read> {
    let needs = chats?.as_array()?.iter().filter(|c| c["needs"] == true);
    let needs = needs.filter_map(|c| {
        let title = c["title"].as_str().unwrap_or_default().to_string();
        Some((c["id"].as_str()?.to_string(), title))
    });
    Some(Read {
        needs: needs.collect(),
        paused: app_paused(pauses?),
    })
}

/// Whether `GET /api/pauses` holds a pause of the whole app.
pub fn app_paused(pauses: &Value) -> bool {
    let mut pauses = pauses["pauses"].as_array().into_iter().flatten();
    pauses.any(|p| p["scope"] == "app")
}

/// What the window keeps of a chat between reads, in memory only.
#[derive(Debug, Clone, Default)]
struct Chat {
    /// In the last read's `needs`.
    needs: bool,
    /// Told since the window was last in front.
    told: bool,
    /// A pause holds it.
    held: bool,
}

/// What the window keeps between reads.
#[derive(Debug, Clone, Default)]
pub struct State {
    chats: BTreeMap<String, Chat>,
    paused: bool,
}

/// How the app started. After an update made while hidden, what waits is taken as seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    AfterUpdate,
    Other,
}

/// What a read decides. Each is recorded as a `gathr.ui.notify` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Show {
        chat: String,
        title: String,
        body: String,
        /// What an app pause let through: `gathr_ask`.
        why: Option<&'static str>,
        /// The chat started needing the person.
        rose: bool,
    },
    Summary {
        /// `start` or `resume`.
        at: &'static str,
        count: usize,
        /// The one chat counted, when there is only one.
        chat: Option<String>,
    },
    Held {
        chat: String,
    },
    Skipped {
        chat: String,
        /// `front`, or `told` for a chat told already.
        why: &'static str,
    },
}

/// What a chat that needs the person says.
const NEEDS_YOU: &str = "Needs you";

/// What to tell the person after the read `now`. `front`: the window is in front now. `visited`:
/// it has been in front since the last read, if only for a moment. `approvals` gives a chat's open
/// approvals, read only during an app pause.
pub fn decide(
    last: Option<&State>,
    start: Start,
    now: &Read,
    front: bool,
    visited: bool,
    mut approvals: impl FnMut(&str) -> Value,
) -> (State, Vec<Notice>) {
    // At a start in front, or after an update made while hidden, what waits counts as seen.
    let baseline = last.is_none() && (front || start == Start::AfterUpdate);
    let before = last.cloned().unwrap_or_default();
    let resumed = before.paused && !now.paused;
    let mut next = State {
        chats: BTreeMap::new(),
        paused: now.paused,
    };
    let (mut notices, mut shown, mut counted) = (Vec::new(), Vec::new(), Vec::new());
    for (id, title) in &now.needs {
        let mut chat = before.chats.get(id).cloned().unwrap_or_default();
        // A visit, however short, clears the marks.
        if visited {
            (chat.told, chat.held) = (false, false);
        }
        let rose = !chat.needs;
        chat.needs = true;
        // Told: not told again until the window has been in front.
        if chat.told && !front {
            if rose {
                notices.push(Notice::Skipped {
                    chat: id.clone(),
                    why: "told",
                });
            }
            next.chats.insert(id.clone(), chat);
            continue;
        }
        if baseline || front {
            if rose && !baseline {
                notices.push(Notice::Skipped {
                    chat: id.clone(),
                    why: "front",
                });
            }
            (chat.told, chat.held) = (false, false);
        } else if resumed && chat.held {
            counted.push(id.clone());
        } else if rose {
            let through = match now.paused {
                true => gathrs_ask(&approvals(id)).then_some(Some("gathr_ask")),
                false => Some(None),
            };
            match through {
                Some(why) => {
                    (chat.told, chat.held) = (true, false);
                    shown.push(Notice::Show {
                        chat: id.clone(),
                        title: title.clone(),
                        body: NEEDS_YOU.to_string(),
                        why,
                        rose,
                    });
                }
                None => {
                    chat.held = true;
                    notices.push(Notice::Held { chat: id.clone() });
                }
            }
        }
        next.chats.insert(id.clone(), chat);
    }
    // A told chat keeps its mark while out of `needs`, until the window has been in front.
    for (id, chat) in &before.chats {
        if !now.needs.contains_key(id) && chat.told && !front && !visited {
            let chat = Chat {
                needs: false,
                held: false,
                ..chat.clone()
            };
            next.chats.insert(id.clone(), chat);
        }
    }
    match (last, shown.len()) {
        (None, 2..) => notices.push(Notice::Summary {
            at: "start",
            count: shown.len(),
            chat: None,
        }),
        _ => notices.extend(shown),
    }
    if !counted.is_empty() {
        for id in &counted {
            let chat = next.chats.get_mut(id).expect("counted from this read");
            (chat.told, chat.held) = (true, false);
        }
        let chat = (counted.len() == 1).then(|| counted[0].clone());
        notices.push(Notice::Summary {
            at: "resume",
            count: counted.len(),
            chat,
        });
    }
    (next, notices)
}

/// "1 chat needs you", "3 chats need you".
pub fn summary_body(count: usize) -> String {
    match count {
        1 => "1 chat needs you".to_string(),
        n => format!("{n} chats need you"),
    }
}

/// Whether a chat's open approvals, as `GET /api/chats/<id>` gives them, hold one Gathr itself
/// asked for: an app pause lets that through.
fn gathrs_ask(chat: &Value) -> bool {
    let mut open = chat["approvals"].as_array().into_iter().flatten();
    open.any(|a| a["why"].as_str().is_some_and(|w| w.starts_with(GATHR_ASKS)))
}
