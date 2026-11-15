//! When the window tells the person outside it (0058): from what the service says each chat waits
//! on, whether something new waits on them in a chat since the window was last in front. The app's
//! thread reads the routes and shows what this decides; a service test feeds it the real ones.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde_json::{Value, json};

use crate::approvals::GATHR_ASKS;

/// How often the window reads the service.
pub const EVERY: Duration = Duration::from_secs(5);

/// The kinds of waiting line that make a chat need the person.
const NEEDS: [&str; 3] = ["sign_in", "approval", "question"];

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
    /// A pause holds something new in it.
    held: bool,
    /// The keys of what waited on the person in it at its last read.
    seen: BTreeSet<String>,
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
        /// What an app pause let through: `sign_in` or `gathr_ask`.
        why: Option<&'static str>,
        /// The chat started needing the person, rather than something new in it.
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

/// What to tell the person after the read `now`. `front`: the window is in front now. `visited`:
/// it has been in front since the last read, if only for a moment. `waiting` gives a chat's waiting
/// lines (`Value::Null` for a read that failed) and `approvals` its open approvals; each is called
/// only for the chats it needs.
pub fn decide(
    last: Option<&State>,
    start: Start,
    now: &Read,
    front: bool,
    visited: bool,
    mut waiting: impl FnMut(&str) -> Value,
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
        // A visit, however short, clears the marks. What waits is still judged against `seen`, so
        // what came after the person left is told.
        if visited {
            (chat.told, chat.held) = (false, false);
        }
        let rose = !chat.needs;
        chat.needs = true;
        // Told: not read again until the window has been in front, whatever comes up in it.
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
        let said = waiting(id);
        let lines = needing(&said);
        let new: Vec<&Value> = lines
            .iter()
            .copied()
            .filter(|l| rose || !chat.seen.contains(&key(l)))
            .collect();
        if !said.is_null() {
            chat.seen = lines.iter().map(|l| key(l)).collect();
        }
        let fresh = rose || !new.is_empty();
        if baseline || front {
            if fresh && !baseline {
                notices.push(Notice::Skipped {
                    chat: id.clone(),
                    why: "front",
                });
            }
            (chat.told, chat.held) = (false, false);
        } else if resumed && chat.held {
            counted.push(id.clone());
        } else if fresh {
            let through = match now.paused {
                true => let_through(&new, || approvals(id)).map(|(why, l)| (Some(why), Some(l))),
                false => Some((None, new.first().copied())),
            };
            match through {
                Some((why, line)) => {
                    (chat.told, chat.held) = (true, false);
                    let body = body(&said, &lines, line);
                    let (chat, title) = (id.clone(), title.clone());
                    shown.push(Notice::Show {
                        chat,
                        title,
                        body,
                        why,
                        rose,
                    });
                }
                None if chat.held => {}
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

/// The waiting lines that make a chat need the person, in the service's order: oldest first.
fn needing(said: &Value) -> Vec<&Value> {
    let lines = said["lines"].as_array().into_iter().flatten();
    let needs = |l: &&Value| l["moves"] == "you" && NEEDS.iter().any(|k| l["kind"] == *k);
    lines.filter(needs).collect()
}

/// What a line is of, whatever its words say, as the quiet rule keys it (`quiet::key`).
fn key(line: &Value) -> String {
    let fields = ["kind", "task", "on", "agent", "since_us"];
    json!(fields.map(|f| &line[f])).to_string()
}

/// What an app pause lets through of the new lines: a sign-in, or an approval Gathr itself asked
/// for, matched to its line by agent. The chat's approvals are read only for a new approval.
fn let_through<'a>(
    new: &[&'a Value],
    approvals: impl FnOnce() -> Value,
) -> Option<(&'static str, &'a Value)> {
    if let Some(line) = new.iter().find(|l| l["kind"] == "sign_in") {
        return Some(("sign_in", line));
    }
    let asks: Vec<&Value> = new
        .iter()
        .copied()
        .filter(|l| l["kind"] == "approval")
        .collect();
    if asks.is_empty() {
        return None;
    }
    let open = approvals();
    let gathrs: Vec<&Value> = open["approvals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["why"].as_str().is_some_and(|w| w.starts_with(GATHR_ASKS)))
        .map(|a| &a["agent"])
        .collect();
    let ask = asks.into_iter().find(|l| gathrs.contains(&&l["agent"]));
    ask.map(|line| ("gathr_ask", line))
}

/// The service's own words for `line`, else for the first line nobody is on, and how many more
/// lines wait on the person.
fn body(said: &Value, lines: &[&Value], line: Option<&Value>) -> String {
    let mut all = said["lines"].as_array().into_iter().flatten();
    let first = line.or_else(|| all.find(|l| l["moves"] == "nobody"));
    let what = first
        .and_then(|l| l["what"].as_str())
        .unwrap_or("Needs you");
    match lines.len() - usize::from(line.is_some()) {
        0 => what.to_string(),
        more => format!("{what}\nAnd {more} more"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Value, json};

    use super::*;
    use crate::approvals::GATHR_ASKS;

    fn line(
        kind: &str,
        what: &str,
        agent: &str,
        task: Option<i64>,
        since: i64,
        moves: &str,
    ) -> Value {
        json!({ "kind": kind, "what": what, "on": null, "agent": agent, "task": task,
            "since_us": since, "moves": moves })
    }

    fn question(n: i64) -> Value {
        line(
            "question",
            &format!("A question for you: q{n}"),
            "lead",
            Some(n),
            n,
            "you",
        )
    }

    fn approval(agent: &str, since: i64) -> Value {
        line(
            "approval",
            &format!("{agent} needs your approval"),
            agent,
            None,
            since,
            "you",
        )
    }

    fn sign_in(agent: &str, since: i64) -> Value {
        let what = format!("{agent}'s turn waits for you to sign in");
        line("sign_in", &what, agent, None, since, "you")
    }

    /// An open approval as `GET /api/chats/<id>` gives it.
    fn open(agent: &str, why: Option<&str>) -> Value {
        json!({ "agent": agent, "why": why })
    }

    fn gathrs_ask(agent: &str) -> Value {
        open(
            agent,
            Some(&format!(
                "{GATHR_ASKS} it writes outside the agent's folders"
            )),
        )
    }

    fn show(chat: &str, body: &str, rose: bool) -> Notice {
        let title = format!("Title {chat}");
        let (chat, body) = (chat.to_string(), body.to_string());
        Notice::Show {
            chat,
            title,
            body,
            why: None,
            rose,
        }
    }

    fn let_through(chat: &str, body: &str, why: &'static str, rose: bool) -> Notice {
        match show(chat, body, rose) {
            Notice::Show {
                chat,
                title,
                body,
                rose,
                ..
            } => Notice::Show {
                chat,
                title,
                body,
                why: Some(why),
                rose,
            },
            _ => unreachable!(),
        }
    }

    fn held(chat: &str) -> Notice {
        Notice::Held {
            chat: chat.to_string(),
        }
    }

    fn skipped(chat: &str, why: &'static str) -> Notice {
        Notice::Skipped {
            chat: chat.to_string(),
            why,
        }
    }

    fn summary(at: &'static str, count: usize, chat: Option<&str>) -> Notice {
        Notice::Summary {
            at,
            count,
            chat: chat.map(String::from),
        }
    }

    /// The service as the window reads it, tick by tick: the chats that need the person with
    /// their waiting lines, the open approvals, and the routes or reads that fail.
    struct Room {
        start: Start,
        state: Option<State>,
        lines: BTreeMap<String, Vec<Value>>,
        asks: HashMap<String, Vec<Value>>,
        paused: bool,
        /// The window has been in front since the last tick: the next tick takes it.
        visited: bool,
        chats_fail: bool,
        pauses_fail: bool,
        waiting_fails: bool,
    }

    impl Room {
        fn new(start: Start) -> Room {
            Room {
                start,
                state: None,
                lines: BTreeMap::new(),
                asks: HashMap::new(),
                paused: false,
                visited: false,
                chats_fail: false,
                pauses_fail: false,
                waiting_fails: false,
            }
        }

        /// One tick: what it decided, and the chats whose waiting lines it read.
        fn tick(&mut self, front: bool) -> (Vec<Notice>, Vec<String>) {
            let mut chats: Vec<Value> = self
                .lines
                .keys()
                .map(|c| json!({ "id": c, "title": format!("Title {c}"), "needs": true }))
                .collect();
            chats.push(json!({ "id": "calm", "title": "Calm", "needs": false }));
            let chats = Value::Array(chats);
            let scope = if self.paused { "app" } else { "chat" };
            let pauses = json!({ "pauses": [{ "scope": scope, "chat": "calm" }] });
            let answered = |fails: bool, body| (!fails).then_some(body);
            let now = read(
                answered(self.chats_fail, &chats),
                answered(self.pauses_fail, &pauses),
            );
            let Some(now) = now else {
                return (Vec::new(), Vec::new());
            };
            let mut read_for = Vec::new();
            let (lines, asks, fails) = (&self.lines, &self.asks, self.waiting_fails);
            let waiting = |chat: &str| {
                read_for.push(chat.to_string());
                match fails {
                    true => Value::Null,
                    false => json!({ "lines": lines[chat] }),
                }
            };
            let approvals = |chat: &str| json!({ "approvals": asks.get(chat) });
            let (state, said) = decide(
                self.state.as_ref(),
                self.start,
                &now,
                front,
                std::mem::take(&mut self.visited),
                waiting,
                approvals,
            );
            self.state = Some(state);
            (said, read_for)
        }

        fn needs(&mut self, chat: &str, lines: Vec<Value>) {
            self.lines.insert(chat.to_string(), lines);
        }

        fn add(&mut self, chat: &str, line: Value) {
            self.lines
                .get_mut(chat)
                .expect("the chat needs you")
                .push(line);
        }
    }

    fn chats(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// A room past its first read, with nothing that needs the person.
    fn started() -> Room {
        let mut room = Room::new(Start::Other);
        room.tick(false);
        room
    }

    /// A room paused with each of `held` held for a question.
    fn paused_with(held: &[&str]) -> Room {
        let mut room = started();
        room.paused = true;
        for chat in held {
            room.needs(chat, vec![question(1)]);
        }
        let said = room.tick(false).0;
        assert_eq!(said, held.iter().map(|c| self::held(c)).collect::<Vec<_>>());
        room
    }

    #[test]
    fn the_first_read_after_an_update_restart_shows_nothing() {
        let mut room = Room::new(Start::AfterUpdate);
        for chat in ["x", "y", "z"] {
            room.needs(chat, vec![question(1)]);
        }
        assert_eq!(room.tick(false), (vec![], chats(&["x", "y", "z"])));
        assert_eq!(room.tick(false).0, []);
        room.add("y", question(2));
        let body = "A question for you: q2\nAnd 1 more";
        assert_eq!(room.tick(false).0, [show("y", body, false)]);
    }

    #[test]
    fn the_first_read_after_any_other_start_tells_once() {
        let mut room = Room::new(Start::Other);
        for chat in ["x", "y", "z"] {
            room.needs(chat, vec![question(1)]);
        }
        assert_eq!(room.tick(false).0, [summary("start", 3, None)]);
        assert_eq!(room.tick(false).0, []);

        let mut room = Room::new(Start::Other);
        room.needs("x", vec![question(1)]);
        assert_eq!(
            room.tick(false).0,
            [show("x", "A question for you: q1", true)]
        );

        let mut room = Room::new(Start::Other);
        room.needs("x", vec![question(1)]);
        assert_eq!(room.tick(true).0, []);

        let mut room = Room::new(Start::Other);
        room.paused = true;
        room.needs("x", vec![question(1)]);
        room.needs("y", vec![sign_in("ann", 5)]);
        let signs_in = let_through("y", "ann's turn waits for you to sign in", "sign_in", true);
        assert_eq!(room.tick(false).0, [held("x"), signs_in]);
    }

    #[test]
    fn a_chat_that_starts_needing_you_while_the_window_is_hidden_is_shown_once() {
        let mut room = started();
        room.needs("x", vec![approval("ann", 7)]);
        assert_eq!(
            room.tick(false).0,
            [show("x", "ann needs your approval", true)]
        );
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn nothing_is_shown_while_the_window_is_in_front() {
        let mut room = started();
        room.needs("x", vec![approval("ann", 7)]);
        assert_eq!(
            room.tick(true),
            (vec![skipped("x", "front")], chats(&["x"]))
        );
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn a_chat_that_stops_needing_you_and_starts_again() {
        let mut room = started();
        room.needs("x", vec![question(1)]);
        assert_eq!(
            room.tick(false).0,
            [show("x", "A question for you: q1", true)]
        );
        room.lines.remove("x");
        assert_eq!(room.tick(false).0, []);
        room.needs("x", vec![question(1)]);
        assert_eq!(room.tick(false).0, [skipped("x", "told")]);

        assert_eq!(room.tick(true).0, []);
        room.lines.remove("x");
        assert_eq!(room.tick(false).0, []);
        room.needs("x", vec![question(1)]);
        assert_eq!(
            room.tick(false).0,
            [show("x", "A question for you: q1", true)]
        );
    }

    #[test]
    fn an_app_wide_pause_holds_all_but_a_sign_in_and_gathrs_own_ask() {
        let asks = "ann needs your approval";
        let cases = [
            ("a question", vec![question(1)], vec![], held("x")),
            (
                "Claude Code's own approval",
                vec![approval("ann", 7)],
                vec![open("ann", None)],
                held("x"),
            ),
            (
                "a sign-in",
                vec![sign_in("ann", 7)],
                vec![],
                let_through("x", "ann's turn waits for you to sign in", "sign_in", true),
            ),
            (
                "Gathr's own ask",
                vec![approval("ann", 7)],
                vec![gathrs_ask("ann")],
                let_through("x", asks, "gathr_ask", true),
            ),
            (
                "a check Gathr runs",
                vec![approval("ann", 7)],
                vec![open("ann", Some("Gathr runs npm test until it exits 0"))],
                held("x"),
            ),
        ];
        for (case, lines, open, expected) in cases {
            let mut room = started();
            room.paused = true;
            room.needs("x", lines);
            room.asks.insert("x".into(), open);
            assert_eq!(room.tick(false).0, [expected], "{case}");
        }

        // Gathr's ask is matched to the new line by its agent.
        let mut room = started();
        room.needs("x", vec![approval("bo", 5)]);
        room.asks.insert("x".into(), vec![gathrs_ask("bo")]);
        room.tick(true);
        room.paused = true;
        room.add("x", approval("ann", 7));
        room.asks.get_mut("x").unwrap().push(open("ann", None));
        assert_eq!(room.tick(false).0, [held("x")]);

        // Held once; more in it holds nothing more, until a sign-in comes up in it.
        let mut room = paused_with(&["x"]);
        assert_eq!(room.tick(false).0, []);
        room.add("x", question(2));
        assert_eq!(room.tick(false).0, []);
        room.add("x", sign_in("ann", 9));
        let body = "ann's turn waits for you to sign in\nAnd 2 more";
        assert_eq!(
            room.tick(false).0,
            [let_through("x", body, "sign_in", false)]
        );
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn resuming_shows_one_summary_counting_the_held_chats() {
        let mut room = paused_with(&["x", "y", "z"]);
        room.paused = false;
        assert_eq!(room.tick(false).0, [summary("resume", 3, None)]);
        assert_eq!(room.tick(false).0, []);

        let mut room = paused_with(&["x"]);
        room.paused = false;
        assert_eq!(room.tick(false).0, [summary("resume", 1, Some("x"))]);

        let mut room = paused_with(&["x"]);
        room.paused = false;
        assert_eq!(room.tick(true).0, []);

        let mut room = paused_with(&[]);
        room.paused = false;
        assert_eq!(room.tick(false).0, []);

        // Something new in that same tick, in a chat not held, is shown on its own.
        let mut room = paused_with(&["x"]);
        room.paused = false;
        room.needs("y", vec![question(1)]);
        let new = show("y", "A question for you: q1", true);
        assert_eq!(room.tick(false).0, [new, summary("resume", 1, Some("x"))]);

        // A chat shown during the pause for its sign-in isn't counted.
        let mut room = paused_with(&["x"]);
        room.needs("y", vec![sign_in("ann", 5)]);
        room.tick(false);
        room.paused = false;
        assert_eq!(room.tick(false).0, [summary("resume", 1, Some("x"))]);

        // A chat that needed you before the pause, with nothing new during it, isn't counted.
        let mut room = started();
        room.needs("w", vec![question(1)]);
        room.tick(true);
        room.paused = true;
        assert_eq!(room.tick(false).0, []);
        room.paused = false;
        assert_eq!(room.tick(false).0, []);

        // A held chat with something more at resume is counted, not shown on its own.
        let mut room = paused_with(&["x"]);
        room.add("x", question(2));
        room.paused = false;
        assert_eq!(room.tick(false).0, [summary("resume", 1, Some("x"))]);
    }

    #[test]
    fn a_tick_that_could_not_read_both_routes_changes_nothing() {
        let chats = json!([{ "id": "x", "title": "Title x", "needs": true }]);
        let pauses = json!({ "pauses": [] });
        assert!(read(Some(&chats), None).is_none());
        assert!(read(None, Some(&pauses)).is_none());

        // During a pause, a pauses read that fails is no resume.
        let mut room = paused_with(&["x"]);
        room.pauses_fail = true;
        assert_eq!(room.tick(false).0, []);
        room.pauses_fail = false;
        assert_eq!(room.tick(false).0, []);
        room.chats_fail = true;
        room.paused = false;
        assert_eq!(room.tick(false).0, []);

        // A waiting read that fails, in a chat that already needed you, finds nothing new.
        let mut room = started();
        room.needs("x", vec![question(1)]);
        room.tick(true);
        room.waiting_fails = true;
        assert_eq!(room.tick(false).0, []);
        room.waiting_fails = false;
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn the_body_is_the_first_new_line_that_makes_the_chat_need_you_and_how_many_more() {
        let queued = line(
            "queued",
            "ann's turn is queued: paused",
            "ann",
            None,
            1,
            "you",
        );
        let mut room = started();
        room.needs("x", vec![queued, approval("ann", 2), question(3)]);
        let body = "ann needs your approval\nAnd 1 more";
        assert_eq!(room.tick(false).0, [show("x", body, true)]);

        // What was seen isn't named again.
        let mut room = started();
        room.needs("x", vec![question(1)]);
        room.tick(true);
        room.add("x", approval("ann", 5));
        assert_eq!(room.tick(false).0, [show("x", body, false)]);

        let mut room = started();
        room.paused = true;
        room.needs("x", vec![question(1), sign_in("ann", 5)]);
        let body = "ann's turn waits for you to sign in\nAnd 1 more";
        assert_eq!(
            room.tick(false).0,
            [let_through("x", body, "sign_in", true)]
        );

        let mut room = started();
        let quiet = |what| line("idle", what, "lead", None, 1, "nobody");
        room.needs("x", vec![quiet("Nothing moves here"), quiet("Nor here")]);
        assert_eq!(room.tick(false).0, [show("x", "Nothing moves here", true)]);

        let mut room = started();
        room.needs("x", vec![]);
        assert_eq!(room.tick(false).0, [show("x", "Needs you", true)]);

        let mut room = started();
        room.waiting_fails = true;
        room.needs("x", vec![question(1)]);
        assert_eq!(room.tick(false).0, [show("x", "Needs you", true)]);
    }

    #[test]
    fn something_new_in_a_chat_that_already_needs_you_is_told_once_until_the_window_has_been_in_front()
     {
        let mut room = started();
        room.needs("x", vec![question(1)]);
        room.tick(true);
        room.add("x", approval("ann", 5));
        let body = "ann needs your approval\nAnd 1 more";
        assert_eq!(room.tick(false).0, [show("x", body, false)]);
        room.add("x", question(2));
        assert_eq!(room.tick(false), (vec![], vec![]));
        // In front, nothing is shown; q2 counts as seen.
        assert_eq!(room.tick(true).0, [skipped("x", "front")]);

        // A second approval of the same agent and task: a later time.
        room.lines
            .get_mut("x")
            .unwrap()
            .retain(|l| l["kind"] != "approval");
        room.add("x", approval("ann", 8));
        let body = "ann needs your approval\nAnd 2 more";
        assert_eq!(room.tick(false).0, [show("x", body, false)]);

        // The key doesn't hold the words: a renamed agent is nothing new.
        room.tick(true);
        room.lines.get_mut("x").unwrap()[0]["what"] = json!("A question for you, from Lead2: q1");
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn a_visit_between_reads_clears_the_marks_but_counts_nothing_as_seen() {
        let told = || {
            let mut room = started();
            room.needs("x", vec![approval("ann", 7)]);
            let a = show("x", "ann needs your approval", true);
            assert_eq!(room.tick(false).0, [a]);
            room.visited = true;
            room
        };
        // A is answered during the visit; B, later, is told.
        let mut room = told();
        room.lines.remove("x");
        assert_eq!(room.tick(false).0, []);
        room.needs("x", vec![approval("bo", 9)]);
        let b = show("x", "bo needs your approval", true);
        assert_eq!(room.tick(false).0, [b]);
        // A is left: nothing is told again.
        let mut room = told();
        assert_eq!(room.tick(false).0, []);
        assert_eq!(room.tick(false).0, []);
        // B came after the visit, before the read: it is told at that read.
        let mut room = told();
        room.add("x", approval("bo", 9));
        let b = show("x", "bo needs your approval\nAnd 1 more", false);
        assert_eq!(room.tick(false).0, [b]);
        // Visited and in front: as in front.
        let mut room = told();
        room.add("x", question(2));
        assert_eq!(room.tick(true).0, [skipped("x", "front")]);
        room.add("x", approval("bo", 9));
        let b = show("x", "bo needs your approval\nAnd 2 more", false);
        assert_eq!(room.tick(false).0, [b]);
        // A chat held by a pause, then visited, isn't counted at resume.
        let mut room = paused_with(&["x"]);
        room.visited = true;
        room.paused = false;
        assert_eq!(room.tick(false).0, []);
    }

    #[test]
    fn a_summary_counts_its_chats() {
        assert_eq!(
            (summary_body(1), summary_body(3)),
            ("1 chat needs you".into(), "3 chats need you".into())
        );
    }
}
