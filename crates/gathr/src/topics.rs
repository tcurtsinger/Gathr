//! What every turn, task and line said in no turn of a chat is about (0075): the main message it
//! sits under. A main message is the person's message, a task the lead hands out, a question to
//! the person, a plan version, the result of an ask, or anything nothing ties: its own. Read from
//! what the chat records, once per chat (`load`), and resolved with maps: never a query per row.
//!
//! Keys: `t:<task>` (a task or question), `m:<message>` (the person's message, a plan version, a
//! main line of Gathr's), `r:<turn>` (a result), `b:<turn>` (a turn tied to nothing).

mod load;
pub mod record;
mod rows;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::Error;
use crate::event::Event;
pub use load::{Facts, Line, Source, TOPICS_SCHEMA};
pub use rows::Rows;

/// The most links followed from one turn or task: past it, the chain ends there (never seen).
pub const LONGEST: usize = 32;

/// Where something sits: its main message, the kind of fact that tied it, and that fact in words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Topic {
    pub key: String,
    pub fact: &'static str,
    pub why: String,
    /// Nothing ties it: it is its own main message.
    pub alone: bool,
    /// A link it followed leads nowhere: a row that is gone, a loop, a chain too long.
    pub broken: bool,
}

impl Topic {
    fn tied(key: String, fact: &'static str, why: String) -> Topic {
        Topic {
            key,
            fact,
            why,
            alone: false,
            broken: false,
        }
    }

    fn alone(key: String, why: String) -> Topic {
        Topic {
            key,
            fact: "nothing",
            why: format!("tied to nothing: {why}"),
            alone: true,
            broken: false,
        }
    }

    fn broken(key: String, why: String) -> Topic {
        Topic {
            fact: "broken",
            broken: true,
            ..Topic::alone(key, why)
        }
    }
}

/// What following a link found.
enum Found {
    Topic(Topic),
    Missing,
    Loops,
}

/// A chat's topics, worked out once each, as asked for.
pub struct Topics<S = Facts> {
    pub src: S,
    turns: RefCell<HashMap<String, Topic>>,
    tasks: RefCell<HashMap<i64, Topic>>,
    visiting: RefCell<HashSet<String>>,
}

impl Topics<Facts> {
    pub fn of_chat(conn: &Connection, chat: &str) -> Result<Topics, Error> {
        Self::at_schema(conn, chat, crate::db::SCHEMA_VERSION)
    }

    /// From a database at `schema`, as `gathr diag` opens one it doesn't upgrade.
    pub fn at_schema(conn: &Connection, chat: &str, schema: i64) -> Result<Topics, Error> {
        Ok(Topics {
            src: Facts::load(conn, chat, schema)?,
            turns: RefCell::default(),
            tasks: RefCell::default(),
            visiting: RefCell::default(),
        })
    }
}

impl<'c> Topics<Rows<'c>> {
    /// Read one row at a time as a chain asks for it: what a live event uses.
    pub fn on_demand(conn: &'c Connection, chat: &str) -> Topics<Rows<'c>> {
        Topics {
            src: Rows::new(conn, chat),
            turns: RefCell::default(),
            tasks: RefCell::default(),
            visiting: RefCell::default(),
        }
    }
}

impl<S: Source> Topics<S> {
    pub fn turn(&self, id: &str) -> Option<Topic> {
        match self.follow_turn(id, 0) {
            Found::Topic(topic) => Some(topic),
            Found::Missing | Found::Loops => None,
        }
    }

    pub fn task(&self, number: i64) -> Option<Topic> {
        match self.follow_task(number, 0) {
            Found::Topic(topic) => Some(topic),
            Found::Missing | Found::Loops => None,
        }
    }
}

impl Topics<Facts> {
    /// A line Gathr said in no turn.
    pub fn line(&self, line: &Line) -> Topic {
        let f = &self.src;
        let own = format!("m:{}", line.id);
        if let Some(about) = &line.about {
            if about == "main" {
                return Topic::tied(own, "main_line", "a main line of its own".into());
            }
            if let Some(n) = about.strip_prefix("t:").and_then(|n| n.parse().ok()) {
                let why = format!("about task #{n}");
                return self.via(
                    &own,
                    self.follow_task(n, 1),
                    "about",
                    why,
                    format!("task #{n}"),
                );
            }
            if let Some(message) = about.strip_prefix("m:") {
                return match f.asks.contains(message) || f.versions.values().any(|m| m == message) {
                    true => Topic::tied(about.clone(), "about", format!("about message {message}")),
                    false => Topic::broken(own, format!("message {message} not found")),
                };
            }
        }
        if let Some(ask) = f.fixed.get(&line.id) {
            return Topic::tied(
                format!("m:{ask}"),
                "fixed",
                format!("the Fixed line of ask {ask}"),
            );
        }
        if let Some(message) = f.version_lines.get(&line.id) {
            let why = format!("said of plan version {message}");
            return Topic::tied(format!("m:{message}"), "plan_line", why);
        }
        if let Some((ask, task)) = f.held_lines.get(&line.id) {
            match (ask, task) {
                (Some(ask), _) => {
                    let why = format!("a caught change of ask {ask}");
                    return Topic::tied(format!("m:{ask}"), "held_line", why);
                }
                (None, Some(n)) => {
                    let why = format!("a caught change of task #{n}");
                    return self.via(
                        &own,
                        self.follow_task(*n, 1),
                        "held_line",
                        why,
                        format!("task #{n}"),
                    );
                }
                (None, None) => {}
            }
        }
        Topic::alone(own, "a line with nothing recorded about it".into())
    }
}

impl<S: Source> Topics<S> {
    fn follow_turn(&self, id: &str, depth: usize) -> Found {
        if let Some(known) = self.turns.borrow().get(id) {
            return Found::Topic(known.clone());
        }
        let Some(t) = self.src.turn(id) else {
            return Found::Missing;
        };
        if !self.visiting.borrow_mut().insert(id.to_string()) {
            return Found::Loops;
        }
        let topic = match depth > LONGEST {
            true => Topic::broken(
                format!("b:{id}"),
                format!("a chain of more than {LONGEST} links"),
            ),
            false => self.turn_of(&t, depth),
        };
        self.visiting.borrow_mut().remove(id);
        self.turns
            .borrow_mut()
            .insert(id.to_string(), topic.clone());
        Found::Topic(topic)
    }

    fn turn_of(&self, t: &load::Turn, depth: usize) -> Topic {
        let (f, own, next) = (&self.src, format!("b:{}", t.id), depth + 1);
        if let Some(ask) = &t.result_of {
            return Topic::tied(
                format!("r:{}", t.id),
                "result",
                format!("the result of ask {ask}"),
            );
        }
        let opening = t.opening.as_ref();
        if let Some(o) = opening.filter(|o| o.role == "user") {
            return Topic::tied(
                format!("m:{}", o.id),
                "person",
                "opened by the person's message".into(),
            );
        }
        if let Some(q) = f.heard(t) {
            return Topic::tied(
                format!("t:{q}"),
                "heard",
                format!("heard the person's word on #{q}"),
            );
        }
        if let Some(pr) = f.landing(t) {
            return match f.pr_task(pr, None) {
                Some(n) => {
                    let why = format!("PR #{pr} Gathr couldn't land, of #{n}");
                    self.via(
                        &own,
                        self.follow_task(n, next),
                        "landing",
                        why,
                        format!("task #{n}"),
                    )
                }
                None => Topic::broken(own, format!("PR #{pr} has no task in this chat")),
            };
        }
        if let Some((copy, original)) = opening.and_then(|o| o.copy_of.as_ref()) {
            let why = format!("carries message {copy}");
            return match original {
                Some(turn) => self.via(
                    &own,
                    self.follow_turn(turn, next),
                    "copy",
                    why,
                    format!("turn {turn}"),
                ),
                None => Topic::broken(own, format!("message {copy} it carries not found")),
            };
        }
        if let Some(n) = t.task {
            let why = format!("its task #{n}");
            return self.via(
                &own,
                self.follow_task(n, next),
                "task",
                why,
                format!("task #{n}"),
            );
        }
        if let Some(a) = &t.answers {
            let why = format!("answers turn {a}");
            return self.via(
                &own,
                self.follow_turn(a, next),
                "answers",
                why,
                format!("answers {a}"),
            );
        }
        if let Some(ask) = &t.ask {
            return Topic::tied(format!("m:{ask}"), "ask", format!("serves ask {ask}"));
        }
        if let (Some("compaction"), Some(before)) = (t.opened_by.as_deref(), &t.before) {
            let why = format!("a compaction, after turn {before}");
            return self.via(
                &own,
                self.follow_turn(before, next),
                "compaction",
                why,
                format!("turn {before}"),
            );
        }
        Topic::alone(own, "no task, no turn it answers and no ask".into())
    }

    fn follow_task(&self, number: i64, depth: usize) -> Found {
        if let Some(known) = self.tasks.borrow().get(&number) {
            return Found::Topic(known.clone());
        }
        let Some(t) = self.src.task(number) else {
            return Found::Missing;
        };
        let visit = format!("#{number}");
        if !self.visiting.borrow_mut().insert(visit.clone()) {
            return Found::Loops;
        }
        let topic = match depth > LONGEST {
            true => Topic::broken(
                format!("t:{number}"),
                format!("a chain of more than {LONGEST} links"),
            ),
            false => self.task_of(number, &t, depth),
        };
        self.visiting.borrow_mut().remove(&visit);
        self.tasks.borrow_mut().insert(number, topic.clone());
        Found::Topic(topic)
    }

    fn task_of(&self, number: i64, t: &load::Task, depth: usize) -> Topic {
        let (own, next) = (format!("t:{number}"), depth + 1);
        if t.kind == "person" {
            return Topic::tied(own, "question", "a question to the person".into());
        }
        if let Some(v) = t.checks {
            return match self.src.version(v) {
                Some(message) => Topic::tied(
                    format!("m:{message}"),
                    "version",
                    format!("checks plan version {message}"),
                ),
                None => Topic::broken(own, format!("plan version {v} not found")),
            };
        }
        if let Some(p) = t.parent {
            let why = format!("part of #{p}");
            return self.via(
                &own,
                self.follow_task(p, next),
                "parent",
                why,
                format!("task #{p}"),
            );
        }
        // A review given by hand names its PR, not its work: it sits where the PR was built.
        let built =
            t.pr.and_then(|pr| Some((pr, self.src.pr_task(pr, Some(number))?)));
        if let (true, Some((pr, n))) = (t.kind == "review", built) {
            let why = format!("reviews PR #{pr}, of #{n}");
            return self.via(
                &own,
                self.follow_task(n, next),
                "pr",
                why,
                format!("task #{n}"),
            );
        }
        let lead = t.giver.as_ref().is_none_or(|g| self.src.is_lead(g));
        if t.kind == "work" && t.owner.is_some() && lead {
            let why = match t.giver {
                Some(_) => "handed out by the lead",
                None => "given by the person",
            };
            return Topic::tied(own, "handed_out", why.into());
        }
        if let Some(turn) = &t.given_in {
            let why = format!("given in turn {turn}");
            return self.via(
                &own,
                self.follow_turn(turn, next),
                "given_in",
                why,
                format!("turn {turn}"),
            );
        }
        Topic::alone(own, "given in no turn".into())
    }

    /// Where a link led: there, tied by `why`; else `own`, tied to nothing, saying what was missing.
    fn via(
        &self,
        own: &str,
        found: Found,
        fact: &'static str,
        why: String,
        missing: String,
    ) -> Topic {
        match found {
            Found::Topic(topic) => Topic::tied(topic.key, fact, why),
            Found::Missing => Topic::broken(own.to_string(), format!("{missing} not found")),
            Found::Loops => {
                Topic::broken(own.to_string(), format!("a chain that loops at {missing}"))
            }
        }
    }
}

impl Topics<Facts> {
    /// Every turn's, task's and line's topic, in the chat's order.
    pub fn all(&self) -> All {
        let f = &self.src;
        let turns = f
            .turns
            .iter()
            .filter_map(|t| Some((t.id.clone(), self.turn(&t.id)?)));
        let mut numbers: Vec<i64> = f.tasks.keys().copied().collect();
        numbers.sort_unstable();
        let tasks = numbers.into_iter().filter_map(|n| Some((n, self.task(n)?)));
        let lines = f.lines.iter().map(|l| (l.id.clone(), self.line(l)));
        All {
            turns: turns.collect(),
            tasks: tasks.collect(),
            lines: lines.collect(),
        }
    }
}

/// A chat's topics, each with what it is the topic of.
pub struct All {
    pub turns: Vec<(String, Topic)>,
    pub tasks: Vec<(i64, Topic)>,
    pub lines: Vec<(String, Topic)>,
}

/// History with each turn's topic, and each line's said in no turn. A link that leads nowhere is an
/// error event on the request's trace, so the reference the answer carries finds it (0019).
pub fn on_history(
    conn: &Connection,
    chat: &str,
    history: &mut Value,
    trace: &str,
) -> Result<(), Error> {
    let topics = Topics::of_chat(conn, chat)?;
    let All { turns, lines, .. } = topics.all();
    let broken: Vec<String> = turns
        .iter()
        .chain(&lines)
        .filter(|(_, t)| t.broken)
        .map(|(id, t)| format!("{id}: {}", t.why))
        .collect();
    let turns: HashMap<String, Topic> = turns.into_iter().collect();
    let lines: HashMap<String, Topic> = lines.into_iter().collect();
    for turn in history["turns"].as_array_mut().into_iter().flatten() {
        let topic = turn["id"].as_str().and_then(|id| turns.get(id));
        turn["topic"] = topic.map(|t| t.key.clone()).into();
    }
    for message in history["messages"].as_array_mut().into_iter().flatten() {
        if message["turn"].is_null() {
            let topic = message["id"].as_str().and_then(|id| lines.get(id));
            message["topic"] = topic.map(|t| t.key.clone()).into();
        }
    }
    if !broken.is_empty() {
        Event::point("gathr.chat.topics")
            .trace(trace)
            .chat(chat)
            .error("unresolved")
            .attr("gathr.topic.broken", broken.len())
            .attr("why", broken.into_iter().take(5).collect::<Vec<_>>())
            .insert(conn)?;
    }
    Ok(())
}

/// Each listed task's topic, followed one row at a time: the list goes out live after every change.
pub fn on_tasks(
    conn: &Connection,
    chat: &str,
    tasks: &mut [crate::tasks::Task],
) -> Result<(), Error> {
    let topics = Topics::on_demand(conn, chat);
    for t in tasks {
        t.topic = topics.task(t.number).map(|topic| topic.key);
    }
    topics.src.read()
}

/// The topics of lines said in no turn, for the live socket: the chat read once, as history reads
/// it. Lines are a few a task, so a read of the whole chat each time costs little.
pub fn of_lines(
    conn: &Connection,
    chat: &str,
    ids: &[&str],
) -> Result<HashMap<String, String>, Error> {
    let topics = Topics::of_chat(conn, chat)?;
    let lines = topics
        .src
        .lines
        .iter()
        .filter(|l| ids.contains(&l.id.as_str()));
    Ok(lines.map(|l| (l.id.clone(), topics.line(l).key)).collect())
}

/// One turn's topic, for the live socket, from its own chain: none for a turn that isn't the chat's.
pub fn of_turn(conn: &Connection, chat: &str, turn: &str) -> Result<Option<String>, Error> {
    Ok(of_turn_counted(conn, chat, turn)?.0)
}

/// `of_turn`, with the steps SQLite took to work it out.
pub fn of_turn_counted(
    conn: &Connection,
    chat: &str,
    turn: &str,
) -> Result<(Option<String>, i64), Error> {
    let topics = Topics::on_demand(conn, chat);
    let topic = topics.turn(turn).map(|t| t.key);
    topics.src.read()?;
    Ok((topic, topics.src.steps()))
}
