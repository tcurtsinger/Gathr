//! The person's open tasks behind F9's proof: the team chat's lead asks five questions together,
//! one of each form, one filed for a member's request and three with files, and the lead asks one
//! more in a chat of its own, so the Tasks list spans two chats.

use rusqlite::params;
use serde_json::json;

use super::Seeder;
use crate::Error;

/// What the lead asks: what, why, form, choices (its pick first) and a file's name and text.
type Question = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(&'static str, &'static str)>,
);

const QUESTIONS: [Question; 5] = [
    (
        "What should the reconnect notice say?",
        "Rowan needs it for the banner. Short wins: it shows on every drop.",
        "choice",
        &["Reconnecting…", "Connection lost", "Trying again"],
        Some((
            "notice.md",
            "# Reconnect notice\n\nThree wordings, shown where the banner sits.\n",
        )),
    ),
    (
        "Which interruptions should the replay cover?",
        "Each one adds a fixture and about a minute of CI.",
        "choices",
        &[
            "Network drop",
            "Sleep and wake",
            "Service restart",
            "Engine crash",
        ],
        Some((
            "replay-plan.md",
            "# Replay plan\n\n- Network drop\n- Sleep and wake\n",
        )),
    ),
    (
        "Keep the original request deadline?",
        "A reconnect restarts the clock today.",
        "yesno",
        &[],
        None,
    ),
    (
        "What should we say if reconnecting takes longer?",
        "After 30 seconds the banner needs a second line.",
        "text",
        &[],
        None,
    ),
    (
        "Check the reconnect notice in the preview",
        "Open it at both widths and say Done.",
        "done",
        &[],
        Some((
            "preview.html",
            "<!doctype html><title>Preview</title><p>Reconnecting…</p>\n",
        )),
    ),
];

impl Seeder<'_> {
    pub(super) fn asks(&mut self) -> Result<(), Error> {
        let team = self.team()?;
        let chat: String = self.conn.query_row(
            "SELECT id FROM chats WHERE title = 'Seed: team of five' ORDER BY created_ts DESC LIMIT 1",
            [],
            |r| r.get(0),
        )?;
        let mut place: i64 = self.conn.query_row(
            "SELECT coalesce(max(position), 0) FROM turns WHERE chat_id = ?1",
            [&chat],
            |r| r.get(0),
        )?;
        let turn = self.team_turn(&chat, &team[0], (None, None), &mut place)?;
        self.answer(&turn, 40)?;
        // Rowan's need, checked by the lead and filed as the first question.
        let request = self.ask(&chat, (&team[1], Some(&team[0])), None, &QUESTIONS[0], None)?;
        self.conn.execute(
            "UPDATE tasks SET state = 'held', reason = ?2 WHERE number = ?1",
            params![
                request,
                format!("done, waiting on #{} (the person)", request + 1)
            ],
        )?;
        for (i, q) in QUESTIONS.iter().enumerate() {
            let from = (i == 0).then_some(request);
            self.ask(&chat, (&team[0], None), Some(&turn.id), q, from)?;
        }
        let alone = self.chat("Seed: release notes")?;
        self.conn.execute(
            "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, 'supervisor', ?3)",
            params![alone, team[0], self.ts],
        )?;
        let mut place = 0;
        let turn = self.team_turn(&alone, &team[0], (None, None), &mut place)?;
        self.answer(&turn, 30)?;
        let q = (
            "Approve the release note wording?",
            "It goes out with the next build.",
            "yesno",
            &[][..],
            None,
        );
        self.ask(&alone, (&team[0], None), Some(&turn.id), &q, None)?;
        Ok(())
    }

    /// A question from `giver`: for the person when `owner` is none, else a request to that owner.
    fn ask(
        &mut self,
        chat: &str,
        (giver, owner): (&str, Option<&str>),
        given_in: Option<&str>,
        (what, why, form, choices, file): &Question,
        request: Option<i64>,
    ) -> Result<i64, Error> {
        self.ts += 1_000_000;
        let kind = if owner.is_some() {
            "person_request"
        } else {
            "person"
        };
        let choices: Vec<_> = choices.iter().map(|l| json!({ "label": l })).collect();
        let files = match (file, owner) {
            (Some((name, text)), None) => {
                let dir = self.files.join(format!("seed-{}", crate::ids::uuid()));
                std::fs::create_dir_all(&dir)?;
                let path = dir.join(format!("0-{name}"));
                std::fs::write(&path, text)?;
                let file =
                    json!({ "name": name, "size": text.len(), "path": path.display().to_string() });
                Some(json!([file]).to_string())
            }
            _ => None,
        };
        self.conn.execute(
            "INSERT INTO tasks (chat_id, giver, owner, given_in, kind, what, why, state, form, choices,
                                files, request, created_ts, updated_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                chat,
                giver,
                owner,
                given_in,
                kind,
                what,
                why,
                form,
                (!choices.is_empty()).then(|| json!(choices).to_string()),
                files,
                request,
                self.ts
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
}
