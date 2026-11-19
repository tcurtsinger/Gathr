//! The rule's facts one row at a time, as a chain asks for them (0075): a live event's topic reads
//! its turn's or task's own chain, each link by an index, never the whole chat. A read that fails is
//! kept, and `read` gives it back.

use std::cell::{Cell, RefCell};

use rusqlite::{Connection, OptionalExtension, Params, Row, StatementStatus, params};

use super::load::{Opening, REPLY_COLUMNS, REPLY_JOINS, Source, Task, Turn, carried_at, reply_at};
use crate::Error;

pub struct Rows<'c> {
    conn: &'c Connection,
    chat: String,
    steps: Cell<i64>,
    failed: RefCell<Option<rusqlite::Error>>,
}

impl<'c> Rows<'c> {
    pub fn new(conn: &'c Connection, chat: &str) -> Rows<'c> {
        Rows {
            conn,
            chat: chat.to_string(),
            steps: Cell::new(0),
            failed: RefCell::new(None),
        }
    }

    /// The steps SQLite took for every read so far.
    pub fn steps(&self) -> i64 {
        self.steps.get()
    }

    /// Whether every read worked: the first that didn't, else nothing.
    pub fn read(&self) -> Result<(), Error> {
        match self.failed.borrow_mut().take() {
            Some(e) => Err(e.into()),
            None => Ok(()),
        }
    }

    /// One row of `sql`, or none; a failure is kept for `read`.
    fn one<T>(
        &self,
        sql: &str,
        args: impl Params,
        row: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Option<T> {
        let found = self.conn.prepare_cached(sql).and_then(|mut stmt| {
            let found = stmt.query_row(args, row).optional();
            self.steps
                .set(self.steps.get() + i64::from(stmt.reset_status(StatementStatus::VmStep)));
            found
        });
        match found {
            Ok(found) => found,
            Err(e) => {
                self.failed.borrow_mut().get_or_insert(e);
                None
            }
        }
    }

    /// The first message the person or Gathr put in the turn, by `messages_opening`.
    fn opening(&self, turn: &str) -> Option<Opening> {
        let sql = format!(
            "SELECT m.id, m.role, m.copy_of, o.turn_id, {REPLY_COLUMNS}
             FROM messages m LEFT JOIN messages o ON o.id = m.copy_of {REPLY_JOINS}
             WHERE m.turn_id = ?1 AND m.role IN ('user', 'gathr') ORDER BY m.ts, m.rowid LIMIT 1"
        );
        self.one(&sql, [turn], |r| {
            Ok(Opening {
                id: r.get(0)?,
                role: r.get(1)?,
                copy_of: carried_at(r, r.get(2)?, 3, 11)?,
                reply: reply_at(r, 4)?,
            })
        })
    }

    /// The agent's turn before this one in the chat, by `turns_by_chat`.
    fn before(&self, agent: &str, (created, rowid): (i64, i64)) -> Option<String> {
        let sql = "SELECT id FROM turns WHERE chat_id = ?1 AND agent_id = ?2
                   AND (created_ts < ?3 OR (created_ts = ?3 AND rowid < ?4))
                   ORDER BY created_ts DESC, rowid DESC LIMIT 1";
        self.one(sql, params![self.chat, agent, created, rowid], |r| r.get(0))
    }
}

impl Source for Rows<'_> {
    fn turn(&self, id: &str) -> Option<Turn> {
        let sql =
            "SELECT id, agent_id, task, answers, ask_id, result_of, opened_by, created_ts, rowid
                   FROM turns WHERE id = ?1 AND chat_id = ?2";
        let (mut turn, at): (Turn, (i64, i64)) = self.one(sql, params![id, self.chat], |r| {
            let turn = Turn {
                id: r.get(0)?,
                agent: r.get(1)?,
                task: r.get(2)?,
                answers: r.get(3)?,
                ask: r.get(4)?,
                result_of: r.get(5)?,
                opened_by: r.get(6)?,
                opening: None,
                before: None,
            };
            Ok((turn, (r.get(7)?, r.get(8)?)))
        })?;
        turn.opening = self.opening(id);
        if let (Some("compaction"), Some(agent)) = (turn.opened_by.as_deref(), &turn.agent) {
            turn.before = self.before(agent, at);
        }
        Some(turn)
    }

    fn task(&self, number: i64) -> Option<Task> {
        let sql = "SELECT kind, giver, owner, parent, checks, given_in, pr FROM tasks WHERE number = ?1 AND chat_id = ?2";
        self.one(sql, params![number, self.chat], |r| {
            Ok(Task {
                kind: r.get(0)?,
                giver: r.get(1)?,
                owner: r.get(2)?,
                parent: r.get(3)?,
                checks: r.get(4)?,
                given_in: r.get(5)?,
                pr: r.get(6)?,
            })
        })
    }

    /// By `tasks_heard`; an earlier Discuss is kept only in its question's answer, so a turn the
    /// person's word opened (or one from before turns said what opened them) also looks there.
    fn heard(&self, turn: &Turn) -> Option<i64> {
        let sql = "SELECT min(number) FROM tasks WHERE heard_in = ?1 AND kind = 'person'";
        let latest: Option<i64> = self.one(sql, [&turn.id], |r| r.get(0)).flatten();
        if !matches!(turn.opened_by.as_deref(), None | Some("answered")) {
            return latest;
        }
        let sql = "SELECT min(number) FROM tasks WHERE chat_id = ?1 AND kind = 'person' AND instr(answer, ?2) > 0";
        let said = format!("\"turn\":\"{}\"", turn.id);
        let discussed: Option<i64> = self
            .one(sql, params![self.chat, said], |r| r.get(0))
            .flatten();
        latest.into_iter().chain(discussed).min()
    }

    fn landing(&self, turn: &Turn) -> Option<i64> {
        if !matches!(turn.opened_by.as_deref(), None | Some("landing")) {
            return None;
        }
        let sql = "SELECT pr FROM landings WHERE chat_id = ?1 AND turn = ?2";
        self.one(sql, params![self.chat, turn.id], |r| r.get(0))
    }

    /// By `tasks_by_pr`.
    fn pr_task(&self, pr: i64, not: Option<i64>) -> Option<i64> {
        let sql =
            "SELECT min(number) FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND number IS NOT ?3";
        self.one(sql, params![self.chat, pr, not], |r| r.get(0))
            .flatten()
    }

    fn version(&self, id: i64) -> Option<String> {
        let sql = "SELECT turn_id FROM plan_versions WHERE id = ?1";
        self.one(sql, [id], |r| r.get(0))
    }

    /// By `plans_by_chat` and the versions' `(plan_id, n)`: the chat's few versions, never all.
    fn wrote(&self, turn: &Turn) -> Option<i64> {
        let sql = "SELECT min(v.n) FROM plans p JOIN plan_versions v ON v.plan_id = p.id
                   WHERE p.chat_id = ?1 AND v.turn_id = ?2";
        self.one(sql, params![self.chat, turn.id], |r| r.get(0))
            .flatten()
    }

    fn is_lead(&self, agent: &str) -> bool {
        let sql = "SELECT 1 FROM chat_members WHERE chat_id = ?1 AND agent_id = ?2 AND role = 'supervisor'";
        self.one(sql, params![self.chat, agent], |r| r.get::<_, i64>(0))
            .is_some()
    }
}
