//! When the window tells the person outside it (0058): from the chat list's `needs`, whether a chat
//! started needing them since the window was last in front; from its `working`, whether a chat's
//! agents have all stopped. The app's thread reads the routes and shows what this decides.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use crate::approvals::GATHR_ASKS;

/// How often the window reads the service.
pub const EVERY: Duration = Duration::from_secs(5);

/// The chats that need the person and those a turn of runs or waits to start, each with its title;
/// and whether the whole app is paused.
#[derive(Debug)]
pub struct Read {
    pub needs: BTreeMap<String, String>,
    pub working: BTreeMap<String, String>,
    pub paused: bool,
}

/// A read of `GET /api/chats` and `GET /api/pauses`: none unless both answered, so a pauses read
/// that failed during a pause never looks like a resume.
pub fn read(chats: Option<&Value>, pauses: Option<&Value>) -> Option<Read> {
    let chats = chats?.as_array()?;
    let pauses = pauses?;
    Some(Read {
        needs: titled(chats, |c| c["needs"] == true),
        // A teammate's turn queued behind its agent's work elsewhere is work still to come.
        working: titled(chats, |c| c["working"] == true || c["queued"] == true),
        paused: app_paused(pauses),
    })
}

/// The chats `pick` takes, each with its title.
fn titled(chats: &[Value], pick: impl Fn(&Value) -> bool) -> BTreeMap<String, String> {
    let chats = chats.iter().filter(|c| pick(c));
    let chats = chats.filter_map(|c| {
        let title = c["title"].as_str().unwrap_or_default().to_string();
        Some((c["id"].as_str()?.to_string(), title))
    });
    chats.collect()
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
    /// The chats working at the last read, with their titles.
    working: BTreeMap<String, String>,
    /// The chats that stopped at the last read, told if the next still finds them stopped.
    stopping: BTreeMap<String, String>,
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
        /// The chat started needing the person; false when its work stopped.
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

/// What a chat whose agents have all stopped says.
const STOPPED: &str = "Work stopped";

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
        working: now.working.clone(),
        stopping: BTreeMap::new(),
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
    // A chat that stopped is told at the next read that still finds it stopped: between one
    // agent's turn and the teammate it named, no turn of the chat runs for a moment.
    let quiet = |id: &String| {
        let busy = now.working.contains_key(id) || now.needs.contains_key(id);
        !busy && !now.paused && !front && !visited
    };
    for (id, title) in before.stopping.iter().filter(|(id, _)| quiet(id)) {
        notices.push(Notice::Show {
            chat: id.clone(),
            title: title.clone(),
            body: STOPPED.to_string(),
            why: None,
            rose: false,
        });
    }
    let stopped = before.working.iter().filter(|(id, _)| quiet(id));
    next.stopping = stopped.map(|(id, t)| (id.clone(), t.clone())).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A read with chat `c` working or not, and what else holds it.
    fn read(working: bool, needs: bool, paused: bool) -> Read {
        let c = || BTreeMap::from([("c".to_string(), "Plan".to_string())]);
        Read {
            working: if working { c() } else { BTreeMap::new() },
            needs: if needs { c() } else { BTreeMap::new() },
            paused,
        }
    }

    /// The reads in turn, hidden, with a visit before the read at `visit`: each read's stop notices.
    fn told(reads: &[Read], visit: Option<usize>) -> Vec<usize> {
        let mut state = None;
        let mut told = Vec::new();
        for (at, now) in reads.iter().enumerate() {
            let visited = visit == Some(at);
            let (next, notices) = decide(state.as_ref(), Start::Other, now, false, visited, |_| {
                Value::Null
            });
            let stops = notices
                .iter()
                .filter(|n| matches!(n, Notice::Show { body, .. } if body == STOPPED));
            told.extend(stops.map(|_| at));
            state = Some(next);
        }
        told
    }

    const NONE: [usize; 0] = [];

    #[test]
    fn a_chat_with_a_turn_waiting_to_start_reads_as_working() {
        let chats =
            serde_json::json!([{ "id": "c", "title": "Plan", "working": false, "queued": true }]);
        let now = super::read(Some(&chats), Some(&serde_json::json!({ "pauses": [] })));
        assert!(now.expect("both answered").working.contains_key("c"));
    }

    #[test]
    fn a_chat_that_stops_working_is_told_once_at_the_second_read_that_finds_it_stopped() {
        let off = || read(false, false, false);
        let on = || read(true, false, false);
        assert_eq!(told(&[on(), off(), off(), off()], None), [2]);
        // The gap between one agent's turn and the teammate it named.
        assert_eq!(told(&[on(), off(), on(), off()], None), NONE);
        // Never at a start: nothing was working at a read before it.
        assert_eq!(told(&[off(), off(), off()], None), NONE);
    }

    #[test]
    fn a_stop_is_not_told_when_the_chat_needs_the_person_or_the_app_is_paused_or_it_was_seen() {
        let on = || read(true, false, false);
        let stop = |needs, paused| read(false, needs, paused);
        let off = || stop(false, false);
        assert_eq!(told(&[on(), stop(true, false), off()], None), NONE);
        assert_eq!(told(&[on(), off(), stop(true, false)], None), NONE);
        assert_eq!(told(&[on(), off(), stop(false, true)], None), NONE);
        assert_eq!(told(&[on(), off(), off()], Some(1)), NONE);
        assert_eq!(told(&[on(), off(), off()], Some(2)), NONE);
    }
}
