//! The team chat behind 0075's proof: a week of a team at work on a notes app, about 2,000 messages
//! under some 70 main messages, linked the way the service links them. The person's asks; the lead's
//! plans, one sent back and one whose check left findings; the tasks it hands out, their reviews and
//! Gathr's fixes after them; the person's answers to the lead's questions, a Discuss among them;
//! agents' talk, likes, a quiet room's wake, a wait the lead set; the person's message into a running
//! turn, and one to two agents picked; each ask's result. Beside it, a chat of one agent with a reply
//! in a thread (0078). Every name and word is made up.

use rusqlite::params;
use serde_json::json;

use super::{Seeder, Turn};
use crate::Error;
use crate::ids::ulid;
use crate::words::Opened;

mod words;

use words::{ASKS, Ask, COMMANDS, FILES, NAMES, SAID};

/// Tool calls in a member's turn on a task: this many and up to 9 more.
const TOOLS_FROM: u64 = 51;

/// Where the chat is: its id, the team, each agent's place in it, and the next turn's place.
struct Room {
    chat: String,
    team: Vec<String>,
    place: i64,
}

impl Seeder<'_> {
    pub(super) fn threads_chat(&mut self) -> Result<(), Error> {
        let team = self.team()?;
        let chat = self.chat("Seed: team at work")?;
        for (agent, role) in
            team.iter()
                .zip(["supervisor", "member", "member", "member", "reviewer"])
        {
            self.conn.execute(
                "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, ?3, ?4)",
                params![chat, agent, role, self.ts],
            )?;
        }
        let mut room = Room {
            chat,
            team,
            place: 0,
        };
        for (i, ask) in ASKS.iter().enumerate() {
            self.an_ask(&mut room, i, ask)?;
        }
        Ok(())
    }

    fn an_ask(&mut self, room: &mut Room, i: usize, a: &Ask) -> Result<(), Error> {
        let lead = room.team[0].clone();
        let turn = self.turn_as(room, &lead, (None, None), Opened::Person, None)?;
        let ask = self.put(&room.chat, Some(&turn.id), "user", a.said, (None, None))?;
        self.conn.execute(
            "INSERT INTO asks (id, chat_id, size, sized_by, sized_in, sized_ts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![ask, room.chat, if a.plan { "new" } else { "repair" }, lead, turn.id, self.ts],
        )?;
        self.conn.execute(
            "UPDATE turns SET ask_id = ?2 WHERE id = ?1",
            [&turn.id, &ask],
        )?;
        self.tools(&turn, 3)?;
        self.put(
            &room.chat,
            Some(&turn.id),
            "assistant",
            a.answer,
            (None, None),
        )?;
        if i == 7 {
            self.sent_while_running(room, &turn)?;
        }
        if a.plan {
            self.plans(room, &turn, &ask, a)?;
        }
        let mut given = Vec::new();
        for (k, (what, why)) in a.parts.iter().enumerate() {
            let who = 1 + (i + k) % 3;
            given.push(self.hand_out(room, (&turn.id, &ask), who, (what, why))?);
        }
        if let Some((what, choices)) = a.question {
            self.question(room, (&turn.id, &ask), what, choices, i.is_multiple_of(2))?;
        }
        if i == 2 {
            self.talk(room, (i, &ask), &given)?;
            self.wake(room, &ask, given[2])?;
        }
        if i == 3 {
            self.waited(room, (&turn.id, &ask))?;
        }
        for (k, &n) in given.iter().enumerate() {
            let who = 1 + (i + k) % 3;
            self.work_on(room, (&turn.id, &ask), n, who)?;
            if k == 0 {
                self.review(room, &ask, n, who, i % 2 == 1)?;
            }
            if (i, k) == (6, 0) {
                self.heard_part(room, (&turn.id, &ask), n)?;
            }
        }
        if i == 4 {
            self.like(room, &ask)?;
        }
        self.closing(room, (&turn.id, &ask), &given)?;
        if i == 8 {
            self.picked(room)?;
        }
        if i == 1 {
            let mut lines = vec![format!("{} {}", crate::asks::FIXED, a.said)];
            lines.extend(given.iter().map(|n| format!("#{n}: PR #{}", 100 + n)));
            let fixed = self.put(&room.chat, None, "gathr", &lines.join("\n"), (None, None))?;
            self.conn
                .execute("UPDATE asks SET fixed = ?2 WHERE id = ?1", [&ask, &fixed])?;
        }
        Ok(())
    }

    /// A finished turn at the next place, opened by `opened`, serving `ask`.
    fn turn_as(
        &mut self,
        room: &mut Room,
        agent: &str,
        on: (Option<i64>, Option<&str>),
        opened: Opened,
        ask: Option<&str>,
    ) -> Result<Turn, Error> {
        let turn = self.team_turn(&room.chat, agent, on, &mut room.place)?;
        self.conn.execute(
            "UPDATE turns SET opened_by = ?2, opened_version = ?3, ask_id = ?4 WHERE id = ?1",
            params![turn.id, opened.as_str(), opened.version(), ask],
        )?;
        Ok(turn)
    }

    /// A message: in a turn or none, to an agent and a copy of another message: its id.
    fn put(
        &mut self,
        chat: &str,
        turn: Option<&str>,
        role: &str,
        body: &str,
        (to, copy_of): (Option<&str>, Option<&str>),
    ) -> Result<String, Error> {
        self.ts += 1_000_000;
        let id = ulid(self.ts);
        self.conn.execute(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of)
             VALUES (?1, ?2, ?3, ?4, ?5, 'text', ?6, ?7, ?8)",
            params![id, chat, turn, self.ts, role, body, to, copy_of],
        )?;
        self.made.messages += 1;
        Ok(id)
    }

    /// A line Gathr says in no turn, about what `about` names.
    fn line(&mut self, chat: &str, body: &str, about: &str) -> Result<String, Error> {
        let id = self.put(chat, None, "gathr", body, (None, None))?;
        self.conn
            .execute("UPDATE messages SET about = ?2 WHERE id = ?1", [&id, about])?;
        Ok(id)
    }

    /// `n` of the agent's tool calls in its turn, on the app's files.
    fn tools(&mut self, turn: &Turn, n: u64) -> Result<(), Error> {
        for _ in 0..n {
            let pick = self.next() as usize;
            let (tool, text) = match pick % 4 {
                0 => ("Bash", COMMANDS[pick % COMMANDS.len()]),
                1 => ("Edit", FILES[pick % FILES.len()]),
                _ => ("Read", FILES[pick % FILES.len()]),
            };
            self.ts += 2_000_000;
            let body = json!({ "tool": tool, "status": "ok", "duration_ms": 1_500 + pick % 4_000,
                "input": crate::steps::input(text, None) });
            self.message(turn, "assistant", "tool", &body.to_string())?;
        }
        Ok(())
    }

    /// The lead's plan: on the first planned ask a version whose check left findings, which the lead
    /// hears back on and answers in the next; on the second a version sent back; then the one
    /// approved. Each is checked by the Reviewer in a turn of its own, with Gathr's lines.
    fn plans(&mut self, room: &mut Room, turn: &Turn, ask: &str, a: &Ask) -> Result<(), Error> {
        let (lead, rev) = (room.team[0].clone(), room.team[4].clone());
        self.conn.execute(
            "INSERT INTO plans (chat_id, ask_id, lead, created_ts) VALUES (?1, ?2, ?3, ?4)",
            params![room.chat, ask, lead, self.ts],
        )?;
        let plan = self.conn.last_insert_rowid();
        let findings = a.said.starts_with("Add tags");
        let versions = if findings || a.said.starts_with("Import") {
            2
        } else {
            1
        };
        let mut wrote = turn.id.clone();
        for n in 1..=versions {
            let title = a.said.split(',').next().unwrap_or(a.said);
            let text = format!(
                "Plan v{n}: {title}. {} parts, each with a test.",
                a.parts.len()
            );
            let message = self.put(&room.chat, Some(&wrote), "assistant", &text, (None, None))?;
            self.conn.execute(
                "INSERT INTO plan_versions (plan_id, n, message_id, turn_id, title, text, created_ts, sent_ts)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![plan, n, message, wrote, title, text, self.ts],
            )?;
            let version = self.conn.last_insert_rowid();
            for (k, (what, why)) in a.parts.iter().enumerate() {
                self.conn.execute(
                    "INSERT INTO plan_parts (version_id, n, owner, what, why, done, proof, place, size, size_lines)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'It works for every note', 'A test that fails without it',
                             'the project worktree', 'small', 80)",
                    params![version, k as i64 + 1, room.team[1 + k % 3], what, why],
                )?;
            }
            let check = self.given_task(
                room,
                (&lead, &rev, &wrote),
                "review",
                &format!("Check plan v{n}"),
                ask,
            )?;
            self.conn.execute(
                "UPDATE tasks SET checks = ?2 WHERE number = ?1",
                [check, version],
            )?;
            let checking = self.turn_as(
                room,
                &rev,
                (Some(check), Some(&wrote)),
                Opened::PlanCheck,
                Some(ask),
            )?;
            self.put(
                &room.chat,
                Some(&checking.id),
                "gathr",
                &format!("Task #{check} from Mira: Check plan v{n}"),
                (None, None),
            )?;
            self.tools(&checking, 6)?;
            self.put(
                &room.chat,
                Some(&checking.id),
                "assistant",
                "The parts hold together; part 2 needs part 1 first.",
                (None, None),
            )?;
            if findings && n < versions {
                wrote = self.findings(room, (check, version, n), (ask, title))?;
                continue;
            }
            // The plan's lines are tied by the version's own columns, not by `about` (0075).
            let agrees = format!("Ivo agrees with version {n} of the plan \"{title}\".");
            let said = self.put(&room.chat, None, "gathr", &agrees, (None, None))?;
            let decided = if n < versions {
                "sent_back"
            } else {
                "approved"
            };
            let decided_line = match decided {
                "approved" => {
                    let words = format!(
                        "Approved: version {n} of the plan \"{title}\", {} tasks.",
                        a.parts.len()
                    );
                    self.put(&room.chat, None, "gathr", &words, (None, None))?
                }
                _ => {
                    let back = self.turn_as(
                        room,
                        &lead,
                        (None, Some(&wrote)),
                        Opened::SentBack,
                        Some(ask),
                    )?;
                    let words = format!(
                        "The person sent back version {n} of your plan \"{title}\":\nKeep the file dates"
                    );
                    let id = self.put(&room.chat, Some(&back.id), "gathr", &words, (None, None))?;
                    self.tools(&back, 4)?;
                    wrote = back.id.clone();
                    id
                }
            };
            self.conn.execute(
                "UPDATE plan_versions SET check_line = ?2, decided = ?3, decided_ts = ?4, decided_line = ?5 WHERE id = ?1",
                params![version, said, decided, self.ts, decided_line],
            )?;
        }
        Ok(())
    }

    /// A task given in `in_turn`, done, of `ask`: its number.
    fn given_task(
        &mut self,
        room: &Room,
        (giver, owner, in_turn): (&str, &str, &str),
        kind: &str,
        what: &str,
        ask: &str,
    ) -> Result<i64, Error> {
        self.conn.execute(
            "INSERT INTO tasks (chat_id, giver, owner, given_in, kind, what, why, state, finish, heard,
                                created_ts, updated_ts, ask_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Part of the ask', 'done', 'done', 1, ?7, ?7, ?8)",
            params![room.chat, giver, owner, in_turn, kind, what, self.ts, ask],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// The lead hands out one part: a task with its brief.
    fn hand_out(
        &mut self,
        room: &mut Room,
        (turn, ask): (&str, &str),
        who: usize,
        (what, why): (&str, &str),
    ) -> Result<i64, Error> {
        let (lead, owner) = (room.team[0].clone(), room.team[who].clone());
        let n = self.given_task(room, (&lead, &owner, turn), "work", what, ask)?;
        self.conn.execute(
            "UPDATE tasks SET why = ?2, done = 'It works, with a test', proof = 'The test run', place = 'the project worktree'
             WHERE number = ?1",
            params![n, why],
        )?;
        Ok(n)
    }

    /// The owner's turn on task `n`, then Gathr's line that it's done.
    fn work_on(
        &mut self,
        room: &mut Room,
        (gave, ask): (&str, &str),
        n: i64,
        who: usize,
    ) -> Result<(), Error> {
        let owner = room.team[who].clone();
        let turn = self.turn_as(
            room,
            &owner,
            (Some(n), Some(gave)),
            Opened::Brief,
            Some(ask),
        )?;
        let what: String =
            self.conn
                .query_row("SELECT what FROM tasks WHERE number = ?1", [n], |r| {
                    r.get(0)
                })?;
        self.put(
            &room.chat,
            Some(&turn.id),
            "gathr",
            &format!("Task #{n} from Mira: {what}"),
            (None, None),
        )?;
        let tools = TOOLS_FROM + self.next() % 10;
        self.tools(&turn, tools)?;
        let said = SAID[self.next() as usize % SAID.len()];
        self.put(&room.chat, Some(&turn.id), "assistant", said, (None, None))?;
        self.done_line(&room.chat, n)
    }

    fn done_line(&mut self, chat: &str, n: i64) -> Result<(), Error> {
        let task = crate::tasks::get(self.conn, n)?;
        let line = task
            .map(|t| crate::tasks::done_words(self.conn, &t))
            .transpose()?
            .flatten();
        if let Some(line) = line {
            self.line(chat, &line, &format!("t:{n}"))?;
        }
        Ok(())
    }

    /// Gathr gives the Reviewer the PR of task `n`; with `notes`, a fix after it and a second look.
    fn review(
        &mut self,
        room: &mut Room,
        ask: &str,
        n: i64,
        who: usize,
        notes: bool,
    ) -> Result<(), Error> {
        let (builder, rev) = (room.team[who].clone(), room.team[4].clone());
        let pr = 100 + n;
        self.conn
            .execute("UPDATE tasks SET pr = ?2 WHERE number = ?1", [n, pr])?;
        let built: String = self.conn.query_row(
            "SELECT id FROM turns WHERE task = ?1 ORDER BY rowid DESC LIMIT 1",
            [n],
            |r| r.get(0),
        )?;
        let review = self.given_task(
            room,
            (&builder, &rev, &built),
            "review",
            &format!("Review #{pr}"),
            ask,
        )?;
        self.conn.execute(
            "UPDATE tasks SET parent = ?2, pr = ?3 WHERE number = ?1",
            [review, n, pr],
        )?;
        let looked = self.turn_as(
            room,
            &rev,
            (Some(review), Some(&built)),
            Opened::ReviewBrief,
            Some(ask),
        )?;
        self.put(
            &room.chat,
            Some(&looked.id),
            "gathr",
            &format!("Task #{review}: Review #{pr}"),
            (None, None),
        )?;
        self.tools(&looked, 10)?;
        let verdict = match notes {
            true => "One note: the test doesn't cover a note with no tags.",
            false => "Clean. It does what the ask needs and nothing more.",
        };
        self.put(
            &room.chat,
            Some(&looked.id),
            "assistant",
            verdict,
            (None, None),
        )?;
        if notes {
            let fix = self.given_task(
                room,
                (&rev, &builder, &looked.id),
                "work",
                &format!("Fix the review of #{pr}"),
                ask,
            )?;
            self.conn.execute(
                "UPDATE tasks SET parent = ?2, pr = ?3, in_passing = 1 WHERE number = ?1",
                [fix, review, pr],
            )?;
            let fixing = self.turn_as(
                room,
                &builder,
                (Some(fix), Some(&looked.id)),
                Opened::Brief,
                Some(ask),
            )?;
            self.put(
                &room.chat,
                Some(&fixing.id),
                "gathr",
                &format!("Task #{fix} from Ivo: Fix the review of #{pr}"),
                (None, None),
            )?;
            self.tools(&fixing, 12)?;
            self.put(
                &room.chat,
                Some(&fixing.id),
                "assistant",
                "Covered the empty case; the test is in.",
                (None, None),
            )?;
            self.done_line(&room.chat, fix)?;
            let again = self.turn_as(
                room,
                &rev,
                (Some(review), Some(&fixing.id)),
                Opened::HearBack,
                Some(ask),
            )?;
            self.put(
                &room.chat,
                Some(&again.id),
                "gathr",
                &format!("The task you gave is settled:\n- #{fix}: done"),
                (None, None),
            )?;
            self.tools(&again, 4)?;
            self.put(
                &room.chat,
                Some(&again.id),
                "assistant",
                "Second look: clean now.",
                (None, None),
            )?;
        }
        self.done_line(&room.chat, review)
    }

    /// The lead's question to the person, answered in a turn of the lead's; with `discussed`, the
    /// person talked it over first.
    fn question(
        &mut self,
        room: &mut Room,
        (turn, ask): (&str, &str),
        what: &str,
        choices: &[&str],
        discussed: bool,
    ) -> Result<(), Error> {
        let lead = room.team[0].clone();
        self.conn.execute(
            "INSERT INTO tasks (chat_id, giver, given_in, kind, what, why, state, form, choices, created_ts, updated_ts, ask_id)
             VALUES (?1, ?2, ?3, 'person', ?4, 'It decides how the parts are built', 'done', 'choice', ?5, ?6, ?6, ?7)",
            params![
                room.chat,
                lead,
                turn,
                what,
                json!(choices.iter().map(|c| json!({ "label": c })).collect::<Vec<_>>()).to_string(),
                self.ts,
                ask
            ],
        )?;
        let q = self.conn.last_insert_rowid();
        let mut answer = json!({ "pick": [choices[0]], "yes": null, "done": null, "text": null, "by": null, "at": self.ts });
        if discussed {
            let talk =
                self.turn_as(room, &lead, (None, Some(turn)), Opened::Answered, Some(ask))?;
            self.put(
                &room.chat,
                Some(&talk.id),
                "gathr",
                &format!("The person wants to discuss: #{q} {what}\nWhat do other apps do?"),
                (None, None),
            )?;
            self.put(
                &room.chat,
                Some(&talk.id),
                "assistant",
                "Most keep it simple; the first choice matches how the list reads.",
                (None, None),
            )?;
            answer["discussed"] =
                json!([{ "text": "What do other apps do?", "turn": talk.id, "at": self.ts }]);
        }
        let heard = self.turn_as(room, &lead, (None, Some(turn)), Opened::Answered, Some(ask))?;
        self.put(
            &room.chat,
            Some(&heard.id),
            "gathr",
            &format!(
                "The person answered:\n- #{q} {what}: Picked: {}",
                choices[0]
            ),
            (None, None),
        )?;
        self.put(
            &room.chat,
            Some(&heard.id),
            "assistant",
            &format!("{} it is; I've told the team.", choices[0]),
            (None, None),
        )?;
        self.conn.execute(
            "UPDATE tasks SET answer = ?2, heard_in = ?3 WHERE number = ?1",
            params![q, answer.to_string(), heard.id],
        )?;
        Ok(())
    }

    /// The owner of the ask's first task asks the owner of its second, who answers in a turn of its
    /// own.
    fn talk(
        &mut self,
        room: &mut Room,
        (i, ask): (usize, &str),
        given: &[i64],
    ) -> Result<(), Error> {
        let (a, b) = (1 + i % 3, 1 + (i + 1) % 3);
        let (from, to) = (room.team[a].clone(), room.team[b].clone());
        let asking = self.turn_as(
            room,
            &from,
            (Some(given[0]), None),
            Opened::Brief,
            Some(ask),
        )?;
        self.tools(&asking, 5)?;
        let words = format!(
            "{}, does the queue keep its place across a restart too, or only a sleep?",
            NAMES[b]
        );
        let sent = self.put(
            &room.chat,
            Some(&asking.id),
            "assistant",
            &words,
            (Some(&to), None),
        )?;
        let told = self.turn_as(
            room,
            &to,
            (None, Some(&asking.id)),
            Opened::Teammate,
            Some(ask),
        )?;
        let copy = format!("{}: {words}", NAMES[a]);
        self.put(
            &room.chat,
            Some(&told.id),
            "gathr",
            &copy,
            (None, Some(&sent)),
        )?;
        self.tools(&told, 3)?;
        self.put(
            &room.chat,
            Some(&told.id),
            "assistant",
            "Both: it's saved to disk before each upload.",
            (Some(&from), None),
        )?;
        Ok(())
    }

    /// The room went quiet on one task of the ask: the lead is woken, serving that ask.
    fn wake(&mut self, room: &mut Room, ask: &str, task: i64) -> Result<(), Error> {
        let lead = room.team[0].clone();
        let woke = self.turn_as(room, &lead, (None, None), Opened::Wake, Some(ask))?;
        let words = format!("Not moving for 6 minutes: #{task}. Nothing waits on the person.");
        self.put(&room.chat, Some(&woke.id), "gathr", &words, (None, None))?;
        self.put(
            &room.chat,
            Some(&woke.id),
            "assistant",
            "Its turn was held behind the sync test; it runs now.",
            (None, None),
        )?;
        Ok(())
    }

    /// The person likes an agent's message, and the agent hears it in a turn of its own.
    fn like(&mut self, room: &mut Room, ask: &str) -> Result<(), Error> {
        let liked: (String, String) = self.conn.query_row(
            "SELECT m.id, t.agent_id FROM messages m JOIN turns t ON t.id = m.turn_id
             WHERE m.chat_id = ?1 AND m.role = 'assistant' AND m.kind = 'text' ORDER BY m.rowid DESC LIMIT 1",
            [&room.chat],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        self.conn.execute(
            "INSERT INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES (?1, 'person', ?2, ?3)",
            params![liked.0, room.chat, self.ts],
        )?;
        let heard = self.turn_as(room, &liked.1, (None, None), Opened::Liked, Some(ask))?;
        self.put(
            &room.chat,
            Some(&heard.id),
            "gathr",
            "The person liked your message.",
            (None, Some(&liked.0)),
        )?;
        self.put(
            &room.chat,
            Some(&heard.id),
            "assistant",
            "Thanks. Moving on to the next one.",
            (None, None),
        )?;
        Ok(())
    }

    /// The lead hears of one task given in passing before the rest are in: in the ask's thread.
    fn heard_part(
        &mut self,
        room: &mut Room,
        (gave, ask): (&str, &str),
        n: i64,
    ) -> Result<(), Error> {
        let lead = room.team[0].clone();
        let back = self.turn_as(room, &lead, (None, Some(gave)), Opened::HearBack, Some(ask))?;
        let words =
            format!("The task you gave is settled:\n- #{n}: done: the list is 5.2 s of the 6");
        self.put(&room.chat, Some(&back.id), "gathr", &words, (None, None))?;
        let said = "Good numbers: the list is the slow part, so paging goes first.";
        self.put(&room.chat, Some(&back.id), "assistant", said, (None, None))?;
        Ok(())
    }

    /// The Reviewer's check of version `n` leaves two findings; the lead hears back and says how it
    /// answers them: the turn that writes the next version.
    fn findings(
        &mut self,
        room: &mut Room,
        (check, version, n): (i64, i64, i64),
        (ask, title): (&str, &str),
    ) -> Result<String, Error> {
        let notes = [
            (
                "wrong",
                Some("src/tags.rs:40"),
                "Tags aren't saved with the note",
            ),
            ("unasked", None, "Coloured tags weren't asked for"),
        ];
        for (kind, file, says) in notes {
            self.conn.execute(
                "INSERT INTO review_notes (task, reviewed, kind, file, says, created_ts) VALUES (?1, 'v1', ?2, ?3, ?4, ?5)",
                params![check, kind, file, says, self.ts],
            )?;
        }
        self.conn.execute(
            // The next version cancels the check it answers, as writing it does (0062).
            "UPDATE tasks SET state = 'cancelled', finish = 'left', reason = 'left: 2 findings' WHERE number = ?1",
            [check],
        )?;
        let words = format!("Ivo has 2 findings on version {n} of the plan \"{title}\".");
        let line = self.put(&room.chat, None, "gathr", &words, (None, None))?;
        self.conn.execute(
            "UPDATE plan_versions SET check_line = ?2, sent_ts = NULL WHERE id = ?1",
            params![version, line],
        )?;
        let lead = room.team[0].clone();
        let back = self.turn_as(room, &lead, (None, None), Opened::HearBack, Some(ask))?;
        let told = format!("The task you gave is settled:\n- #{check}: left: 2 findings");
        self.put(&room.chat, Some(&back.id), "gathr", &told, (None, None))?;
        self.tools(&back, 2)?;
        let said = "Both findings hold: version 2 saves tags with the note and drops the colours.";
        self.put(&room.chat, Some(&back.id), "assistant", said, (None, None))?;
        Ok(back.id)
    }

    /// A wait the lead set on the ask's work ended: Gathr's words open the lead's turn.
    fn waited(&mut self, room: &mut Room, (turn, ask): (&str, &str)) -> Result<(), Error> {
        let lead = room.team[0].clone();
        let woke = self.turn_as(
            room,
            &lead,
            (None, Some(turn)),
            Opened::WaitFired,
            Some(ask),
        )?;
        let words = "Your wait ended: the search index is rebuilt (4m 10s).";
        self.put(&room.chat, Some(&woke.id), "gathr", words, (None, None))?;
        self.tools(&woke, 2)?;
        let said = "The index is rebuilt and accents match. Sol checks the list next.";
        self.put(&room.chat, Some(&woke.id), "assistant", said, (None, None))?;
        Ok(())
    }

    /// The person writes again while the lead's turn runs: the turn goes on after their message.
    fn sent_while_running(&mut self, room: &Room, turn: &Turn) -> Result<(), Error> {
        let more = "Keep the pinned section collapsible too";
        self.put(&room.chat, Some(&turn.id), "user", more, (None, None))?;
        self.tools(turn, 2)?;
        let said =
            "Collapsible too: it folds like the other sections, and Tamsin's part covers it.";
        self.put(&room.chat, Some(&turn.id), "assistant", said, (None, None))?;
        Ok(())
    }

    /// The person picks two agents in one message (0028): each answers in a turn of its own.
    fn picked(&mut self, room: &mut Room) -> Result<(), Error> {
        let (rowan, sol) = (room.team[1].clone(), room.team[2].clone());
        let first = self.turn_as(room, &rowan, (None, None), Opened::Person, None)?;
        let words = "@Rowan @Sol which of you knows the spell checker's code?";
        let sent = self.put(&room.chat, Some(&first.id), "user", words, (None, None))?;
        let picks =
            json!([{ "at": 0, "len": 6, "agent": rowan }, { "at": 7, "len": 4, "agent": sol }]);
        self.conn.execute(
            "UPDATE messages SET picks = ?2 WHERE id = ?1",
            params![sent, picks.to_string()],
        )?;
        self.tools(&first, 2)?;
        let said =
            "I wrote its tokenizer: code blocks are skipped there, so the bug is in the list view.";
        self.put(&room.chat, Some(&first.id), "assistant", said, (None, None))?;
        let second = self.turn_as(room, &sol, (None, None), Opened::Picked, None)?;
        let copy = format!("The person sent this to you and Rowan.\n\n{words}");
        self.put(
            &room.chat,
            Some(&second.id),
            "gathr",
            &copy,
            (None, Some(&sent)),
        )?;
        let said = "Not me: Rowan has it. I can take the list view after.";
        self.put(
            &room.chat,
            Some(&second.id),
            "assistant",
            said,
            (None, None),
        )?;
        Ok(())
    }

    /// A chat of one agent (0078): drawn by the same rule as a team's, each answer a main message,
    /// and the person's reply to an older answer in that answer's thread.
    pub(super) fn one_agent_chat(&mut self) -> Result<(), Error> {
        let team = self.team()?;
        let chat = self.chat("Seed: one agent")?;
        self.conn.execute(
            "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, 'supervisor', ?3)",
            params![chat, team[0], self.ts],
        )?;
        let mut room = Room {
            chat,
            team,
            place: 0,
        };
        let mira = room.team[0].clone();
        let mut answered = Vec::new();
        for (asked, said) in [
            (
                "Draft the 0.4 release notes",
                "Here is a draft: tags, faster sync, accents in search, and the export fix.",
            ),
            (
                "Add the sync fix too",
                "Added: sync no longer stalls at 99% after the laptop sleeps.",
            ),
        ] {
            let turn = self.turn_as(&mut room, &mira, (None, None), Opened::Person, None)?;
            self.put(&room.chat, Some(&turn.id), "user", asked, (None, None))?;
            self.tools(&turn, 2)?;
            answered.push(self.put(&room.chat, Some(&turn.id), "assistant", said, (None, None))?);
        }
        let turn = self.turn_as(&mut room, &mira, (None, None), Opened::Reply, None)?;
        let reply = self.put(
            &room.chat,
            Some(&turn.id),
            "user",
            "Shorter: one line each",
            (None, None),
        )?;
        self.conn.execute(
            "UPDATE messages SET reply_to = ?2 WHERE id = ?1",
            [&reply, &answered[0]],
        )?;
        let said = "Tags. Faster sync. Accents in search. Export on Windows.";
        self.put(&room.chat, Some(&turn.id), "assistant", said, (None, None))?;
        Ok(())
    }

    /// The lead hears back on everything the ask handed out: the ask's result.
    fn closing(
        &mut self,
        room: &mut Room,
        (gave, ask): (&str, &str),
        given: &[i64],
    ) -> Result<(), Error> {
        let lead = room.team[0].clone();
        let back = self.turn_as(room, &lead, (None, Some(gave)), Opened::HearBack, Some(ask))?;
        self.conn.execute(
            "UPDATE turns SET result_of = ?2 WHERE id = ?1",
            [&back.id, ask],
        )?;
        let mut lines = vec![format!("The {} tasks you gave are settled:", given.len())];
        for n in given {
            lines.push(format!("- #{n}: done"));
        }
        self.put(
            &room.chat,
            Some(&back.id),
            "gathr",
            &lines.join("\n"),
            (None, None),
        )?;
        self.tools(&back, 2)?;
        let words = format!(
            "All {} parts are in, each with its test. Nothing is left open.",
            given.len()
        );
        self.put(
            &room.chat,
            Some(&back.id),
            "assistant",
            &words,
            (None, None),
        )?;
        Ok(())
    }
}
