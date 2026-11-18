//! A chat's facts for its topics, read once: one query a table, by the chat's own indexes.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, Row, StatementStatus};

use crate::Error;

/// The schema whose columns name a result, a landing's turn and a line's subject (migration 0055).
pub const TOPICS_SCHEMA: i64 = 55;

pub struct Turn {
    pub id: String,
    pub agent: Option<String>,
    pub task: Option<i64>,
    pub answers: Option<String>,
    pub ask: Option<String>,
    pub result_of: Option<String>,
    pub opened_by: Option<String>,
    /// The first message the person or Gathr put in it: what opened it.
    pub opening: Option<Opening>,
    /// The agent's turn before it in the chat: where a compaction's words would sit.
    pub before: Option<String>,
}

pub struct Opening {
    pub id: String,
    pub role: String,
    /// A copy of another message (a teammate's, a liked one), and the turn that message is in.
    pub copy_of: Option<(String, Option<String>)>,
}

pub struct Task {
    pub kind: String,
    pub giver: Option<String>,
    pub owner: Option<String>,
    pub parent: Option<i64>,
    pub checks: Option<i64>,
    pub given_in: Option<String>,
    pub pr: Option<i64>,
}

pub struct Line {
    pub id: String,
    pub about: Option<String>,
}

/// One message the person or Gathr put in the chat: its id, turn, role, copy and what it's about.
type Said = (
    String,
    Option<String>,
    String,
    Option<(String, Option<String>)>,
    Option<String>,
);

#[derive(Default)]
pub struct Facts {
    /// In the order they were made.
    pub turns: Vec<Turn>,
    pub at: HashMap<String, usize>,
    pub tasks: HashMap<i64, Task>,
    /// The person's question each turn heard an answer or a Discuss to, the lowest first.
    pub heard: HashMap<String, i64>,
    /// Each plan version's message, and the lines said about a version: its message.
    pub versions: HashMap<i64, String>,
    pub version_lines: HashMap<String, String>,
    pub leads: HashSet<String>,
    pub asks: HashSet<String>,
    /// The "Fixed" line of each repair: its ask.
    pub fixed: HashMap<String, String>,
    /// The lines a caught change was told in: its ask and task.
    pub held_lines: HashMap<String, (Option<String>, Option<i64>)>,
    /// The reviewer's turn Gathr queued for a PR it can't land: the PR.
    pub landings: HashMap<String, i64>,
    /// Gathr's lines in no turn, in order.
    pub lines: Vec<Line>,
    /// The steps SQLite took to read it all: the cost of a chat's topics.
    pub steps: i64,
}

impl Facts {
    /// The chat's facts, from a database at `schema` (before 0055 it records no result, landing turn
    /// or line subject).
    pub fn load(conn: &Connection, chat: &str, schema: i64) -> Result<Facts, Error> {
        let recorded = schema >= TOPICS_SCHEMA;
        let mut f = Facts::default();
        f.load_turns(conn, chat, recorded)?;
        f.load_messages(conn, chat, recorded)?;
        f.load_tasks(conn, chat)?;
        let sql = "SELECT v.id, v.message_id, v.check_line, v.decided_line
                   FROM plan_versions v JOIN plans p ON p.id = v.plan_id WHERE p.chat_id = ?1";
        let versions: Vec<(i64, String, Option<String>, Option<String>)> =
            f.read(conn, sql, chat, |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?;
        for (version, message, check, decided) in versions {
            for line in [check, decided].into_iter().flatten() {
                f.version_lines.insert(line, message.clone());
            }
            f.versions.insert(version, message);
        }
        let sql = "SELECT agent_id FROM chat_members WHERE chat_id = ?1 AND role = 'supervisor'";
        f.leads = f.read(conn, sql, chat, |r| r.get(0))?.into_iter().collect();
        let asks = f.read(
            conn,
            "SELECT id, fixed FROM asks WHERE chat_id = ?1",
            chat,
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )?;
        for (ask, fixed) in asks {
            if let Some(line) = fixed {
                f.fixed.insert(line, ask.clone());
            }
            f.asks.insert(ask);
        }
        let sql =
            "SELECT line, ask_id, task FROM held_turns WHERE chat_id = ?1 AND line IS NOT NULL";
        let held = f.read(conn, sql, chat, |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
        f.held_lines = held.into_iter().collect();
        if recorded {
            let sql = "SELECT turn, pr FROM landings WHERE chat_id = ?1 AND turn IS NOT NULL";
            let landed = f.read(conn, sql, chat, |r| Ok((r.get(0)?, r.get(1)?)))?;
            f.landings = landed.into_iter().collect();
        }
        Ok(f)
    }

    /// The rows of one query on the chat, counting its steps.
    fn read<T>(
        &mut self,
        conn: &Connection,
        sql: &str,
        chat: &str,
        row: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, Error> {
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map([chat], row)?
            .collect::<Result<Vec<T>, _>>()?;
        self.steps += i64::from(stmt.reset_status(StatementStatus::VmStep));
        Ok(rows)
    }

    fn load_turns(&mut self, conn: &Connection, chat: &str, recorded: bool) -> Result<(), Error> {
        let result = if recorded { "result_of" } else { "NULL" };
        let sql = format!(
            "SELECT id, agent_id, task, answers, ask_id, {result}, opened_by FROM turns WHERE chat_id = ?1
             ORDER BY created_ts, rowid"
        );
        let turns = self.read(conn, &sql, chat, |r| {
            Ok(Turn {
                id: r.get(0)?,
                agent: r.get(1)?,
                task: r.get(2)?,
                answers: r.get(3)?,
                ask: r.get(4)?,
                result_of: r.get(5)?,
                opened_by: r.get(6)?,
                opening: None,
                before: None,
            })
        })?;
        let mut last: HashMap<String, String> = HashMap::new();
        for mut turn in turns {
            if let Some(agent) = &turn.agent {
                turn.before = last.insert(agent.clone(), turn.id.clone());
            }
            self.at.insert(turn.id.clone(), self.turns.len());
            self.turns.push(turn);
        }
        Ok(())
    }

    /// The person's messages and Gathr's, in time order: each turn's first is what opened it, and
    /// those in no turn are Gathr's lines.
    fn load_messages(
        &mut self,
        conn: &Connection,
        chat: &str,
        recorded: bool,
    ) -> Result<(), Error> {
        let about = if recorded { "m.about" } else { "NULL" };
        let sql = format!(
            "SELECT m.id, m.turn_id, m.role, m.copy_of, o.turn_id, {about}
             FROM messages m LEFT JOIN messages o ON o.id = m.copy_of
             WHERE m.chat_id = ?1 AND m.role IN ('user', 'gathr') ORDER BY m.ts, m.rowid"
        );
        let said: Vec<Said> = self.read(conn, &sql, chat, |r| {
            let copy: Option<String> = r.get(3)?;
            let copy = match copy {
                Some(of) => Some((of, r.get(4)?)),
                None => None,
            };
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, copy, r.get(5)?))
        })?;
        for (id, turn, role, copy_of, about) in said {
            let Some(turn) = turn else {
                self.lines.push(Line { id, about });
                continue;
            };
            let Some(&i) = self.at.get(&turn) else {
                continue;
            };
            let opening = &mut self.turns[i].opening;
            if opening.is_none() {
                *opening = Some(Opening { id, role, copy_of });
            }
        }
        Ok(())
    }

    fn load_tasks(&mut self, conn: &Connection, chat: &str) -> Result<(), Error> {
        let sql =
            "SELECT number, kind, giver, owner, parent, checks, given_in, pr, heard_in, answer
                   FROM tasks WHERE chat_id = ?1 ORDER BY number";
        let rows = self.read(conn, sql, chat, |r| {
            let task = Task {
                kind: r.get(1)?,
                giver: r.get(2)?,
                owner: r.get(3)?,
                parent: r.get(4)?,
                checks: r.get(5)?,
                given_in: r.get(6)?,
                pr: r.get(7)?,
            };
            let heard: (Option<String>, Option<String>) = (r.get(8)?, r.get(9)?);
            Ok((r.get::<_, i64>(0)?, task, heard))
        })?;
        for (number, task, (heard, answer)) in rows {
            if task.kind == "person" {
                // Each Discuss keeps the turn that heard it (0065); the last one is `heard_in` too.
                let answer =
                    answer.and_then(|a| serde_json::from_str::<serde_json::Value>(&a).ok());
                let discussed = answer
                    .as_ref()
                    .and_then(|a| a["discussed"].as_array().cloned());
                let turns = discussed.into_iter().flatten();
                let turns = turns.filter_map(|d| d["turn"].as_str().map(String::from));
                for turn in heard.into_iter().chain(turns) {
                    self.heard.entry(turn).or_insert(number);
                }
            }
            self.tasks.insert(number, task);
        }
        Ok(())
    }

    /// The task a PR was built in: the lowest-numbered of the chat's tasks on it but `not`.
    pub fn pr_task(&self, pr: i64, not: Option<i64>) -> Option<i64> {
        let on = self
            .tasks
            .iter()
            .filter(|(n, t)| t.pr == Some(pr) && Some(**n) != not);
        on.map(|(n, _)| *n).min()
    }
}
