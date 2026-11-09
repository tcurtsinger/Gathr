//! The team chat behind F8b's proof: a Supervisor, three members and a Reviewer over many rounds,
//! linked the way the service links them (0010, 0028). The lead's turn gives the round's tasks;
//! each member's turn starts with its task and answers that turn; a hear-back ends the round.
//! Every fourth round adds a review after the work, and the last round's first task is restarted
//! after the lead's last word, so its reply comes late.

use rusqlite::params;

use super::{Seeder, Turn};
use crate::Error;
use crate::accounts::key;
use crate::ids::{span_id, trace_id, ulid};

const TEAM: [(&str, &str); 5] = [
    ("Mira", "supervisor"),
    ("Rowan", "member"),
    ("Sol", "member"),
    ("Tamsin", "member"),
    ("Ivo", "reviewer"),
];
const ROUNDS: usize = 20;
const FINISH: &str =
    "Finish with finish_task: done, left (say what's left) or blocked (on whom, or which task).";

/// A task as the seed gives it: its number, owner's place in `TEAM` and what it asks.
struct Given {
    number: i64,
    who: usize,
    what: String,
}

impl Seeder<'_> {
    pub(super) fn team_chat(&mut self) -> Result<(), Error> {
        let team = self.team()?;
        let chat = self.chat("Seed: team of five")?;
        for (agent, (_, role)) in team.iter().zip(TEAM) {
            self.conn.execute(
                "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, ?3, ?4)",
                params![chat, agent, role, self.ts],
            )?;
        }
        let mut place = 0;
        for round in 0..ROUNDS {
            let lead = self.team_turn(&chat, &team[0], (None, None), &mut place)?;
            let words = 10 + (self.next() % 30) as usize;
            let ask = self.text(words);
            self.message(&lead, "user", "text", &ask)?;
            let plan = 40 + self.next() % 80;
            self.answer(&lead, plan as usize)?;
            let mut given = Vec::new();
            for who in 1..4 {
                given.push(self.task(&chat, &team, who, &lead.id, "work", &[])?);
            }
            if round % 4 == 3 {
                let after: Vec<i64> = given.iter().map(|g| g.number).collect();
                given.push(self.task(&chat, &team, 4, &lead.id, "review", &after)?);
            }
            for g in &given {
                self.work(&chat, &team, g, &lead.id, &mut place)?;
            }
            self.hear_back(&chat, &team, &given, &lead.id, &mut place)?;
            if round + 1 == ROUNDS {
                self.conn.execute(
                    "UPDATE tasks SET why = 'Restarted after the round ended' WHERE number = ?1",
                    [given[0].number],
                )?;
                self.work(&chat, &team, &given[0], &lead.id, &mut place)?;
                self.hear_back(&chat, &team, &given[..1], &lead.id, &mut place)?;
            }
        }
        Ok(())
    }

    /// The five agents on a made-up account; a later run reuses them, as names are unique.
    pub(super) fn team(&mut self) -> Result<Vec<String>, Error> {
        let account = "Seed team";
        self.conn.execute(
            "INSERT OR IGNORE INTO accounts (id, engine, name, name_key, created_ts)
             VALUES (?1, 'claude', ?2, ?3, ?4)",
            params![ulid(self.ts), account, key(account), self.ts],
        )?;
        let account: String = self.conn.query_row(
            "SELECT id FROM accounts WHERE name_key = ?1",
            [key(account)],
            |r| r.get(0),
        )?;
        let mut ids = Vec::new();
        for (name, _) in TEAM {
            self.conn.execute(
                "INSERT OR IGNORE INTO agents (id, name, name_key, account_id, created_ts)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ulid(self.ts), name, key(name), account, self.ts],
            )?;
            ids.push(self.conn.query_row(
                "SELECT id FROM agents WHERE name_key = ?1",
                [key(name)],
                |r| r.get(0),
            )?);
        }
        Ok(ids)
    }

    /// A finished turn for `agent` at the next place, with what it works on and the turn it answers.
    pub(super) fn team_turn(
        &mut self,
        chat: &str,
        agent: &str,
        (task, answers): (Option<i64>, Option<&str>),
        place: &mut i64,
    ) -> Result<Turn, Error> {
        self.ts += 20_000_000;
        *place += 1;
        let turn = Turn {
            chat: chat.into(),
            id: ulid(self.ts),
            trace: trace_id(),
            span: span_id(),
            started: self.ts,
        };
        self.conn.execute(
            "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, ended_ts, agent_id, position,
                                task, answers)
             VALUES (?1, ?2, ?3, 'done', ?4, ?4, ?5, ?6, ?7, ?8)",
            params![turn.id, chat, turn.trace, self.ts, agent, *place, task, answers],
        )?;
        let start = self
            .event(&turn, "invoke_agent seed", "start")
            .attr("gen_ai.operation.name", "invoke_agent")
            .attr("gen_ai.agent.id", agent);
        start.insert(self.conn)?;
        self.made.events += 1;
        Ok(turn)
    }

    /// The lead gives a task in its turn `given_in`; it's settled by the time the round ends.
    fn task(
        &mut self,
        chat: &str,
        team: &[String],
        who: usize,
        given_in: &str,
        kind: &str,
        after: &[i64],
    ) -> Result<Given, Error> {
        let what = match kind {
            "review" => "Review the round's changes".to_string(),
            _ => {
                let words = 6 + (self.next() % 10) as usize;
                self.text(words)
            }
        };
        let reviewed = (kind == "review").then(|| format!("{:07x}", self.next() & 0xfff_ffff));
        let result = self.text(8);
        self.conn.execute(
            "INSERT INTO tasks (chat_id, giver, owner, given_in, kind, what, state, reason, finish,
                                reviewed, heard, created_ts, updated_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'done', ?7, 'done', ?8, 1, ?9, ?9)",
            params![
                chat, team[0], team[who], given_in, kind, what, result, reviewed, self.ts
            ],
        )?;
        let number = self.conn.last_insert_rowid();
        for before in after {
            self.conn.execute(
                "INSERT INTO task_after (task, after_task) VALUES (?1, ?2)",
                params![number, before],
            )?;
        }
        Ok(Given { number, who, what })
    }

    /// A member's turn on its task: Gathr's brief, some tools, and the reply.
    fn work(
        &mut self,
        chat: &str,
        team: &[String],
        g: &Given,
        given_in: &str,
        place: &mut i64,
    ) -> Result<(), Error> {
        let turn = self.team_turn(chat, &team[g.who], (Some(g.number), Some(given_in)), place)?;
        let brief = format!(
            "Task #{} from {}: {}\n{FINISH}",
            g.number, TEAM[0].0, g.what
        );
        self.message(&turn, "gathr", "text", &brief)?;
        for _ in 0..self.next() % 6 {
            self.tool_call(&turn)?;
        }
        let words = 60 + self.next() % 240;
        self.answer(&turn, words as usize)
    }

    /// The round's end: Gathr tells the lead its tasks are settled, and the lead's last word.
    fn hear_back(
        &mut self,
        chat: &str,
        team: &[String],
        given: &[Given],
        given_in: &str,
        place: &mut i64,
    ) -> Result<(), Error> {
        let turn = self.team_turn(chat, &team[0], (None, Some(given_in)), place)?;
        let mut lines = vec![match given.len() {
            1 => "The task you gave is settled:".to_string(),
            n => format!("The {n} tasks you gave together are settled:"),
        }];
        for g in given {
            lines.push(format!(
                "- #{} {}, {}: done",
                g.number, TEAM[g.who].0, g.what
            ));
        }
        lines.push("Their replies are in the chat (read_chat). Carry on from here.".into());
        self.message(&turn, "gathr", "text", &lines.join("\n"))?;
        let words = 50 + self.next() % 150;
        self.answer(&turn, words as usize)
    }
}
