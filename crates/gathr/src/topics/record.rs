//! The facts Gathr records where they happen, so a topic needn't be guessed later (0075): the ask a
//! turn about the room as a whole serves, the result of an ask, and the task a PR was built in.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// The ask a turn opened on what it names serves: each thing named is a task, or else a turn, and
/// all of them must be of one ask. None when one is of none, or they are of several.
pub(crate) fn one_ask(
    conn: &Connection,
    named: &[(Option<i64>, Option<&str>)],
) -> Result<Option<String>, Error> {
    let mut found: Option<String> = None;
    for (task, turn) in named {
        let ask: Option<String> = match (task, turn) {
            (Some(n), _) => conn
                .prepare_cached("SELECT ask_id FROM tasks WHERE number = ?1")?
                .query_row([n], |r| r.get(0))
                .optional()?
                .flatten(),
            (None, Some(turn)) => conn
                .prepare_cached("SELECT ask_id FROM turns WHERE id = ?1")?
                .query_row([turn], |r| r.get(0))
                .optional()?
                .flatten(),
            (None, None) => None,
        };
        match (ask, &found) {
            (None, _) => return Ok(None),
            (Some(ask), Some(one)) if ask != *one => return Ok(None),
            (Some(ask), _) => found = Some(ask),
        }
    }
    Ok(found)
}

/// The lead's turn on what `named` names serves their ask, when that is one (question 2 of the plan
/// behind 0075): a quiet room's wake, a silent turn told, stale tasks told.
pub(crate) fn serve_one_ask(
    conn: &Connection,
    turn: &str,
    named: &[(Option<i64>, Option<&str>)],
) -> Result<(), Error> {
    if let Some(ask) = one_ask(conn, named)? {
        conn.prepare_cached("UPDATE turns SET ask_id = ?2 WHERE id = ?1 AND ask_id IS NULL")?
            .execute([turn, &ask])?;
    }
    Ok(())
}

/// The chat's Supervisor's hear-back on no task of its own is the result of its ask when, with it,
/// everything the ask handed out is in: no task of it with an owner is open and no version of its
/// plan waits (0051). Recorded as it is queued, so it is the result from its first word.
pub(crate) fn result(conn: &Connection, chat: &str, giver: &str, turn: &str) -> Result<(), Error> {
    conn.prepare_cached(
        "UPDATE turns SET result_of = ask_id WHERE id = ?1 AND ask_id IS NOT NULL
         AND EXISTS (SELECT 1 FROM chat_members WHERE chat_id = ?2 AND agent_id = ?3 AND role = 'supervisor')
         AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.ask_id = turns.ask_id AND t.owner IS NOT NULL
             AND t.state NOT IN ('done', 'cancelled'))
         AND NOT EXISTS (SELECT 1 FROM plans p JOIN plan_versions v ON v.plan_id = p.id WHERE p.ask_id = turns.ask_id
             AND (v.decided IS NULL OR v.decided = 'sent_back')
             AND v.n = (SELECT max(w.n) FROM plan_versions w WHERE w.plan_id = v.plan_id))",
    )?
    .execute(params![turn, chat, giver])?;
    Ok(())
}

/// The reviewer's turn Gathr queued for a PR it can't land at this head.
pub(crate) fn landing(
    conn: &Connection,
    (chat, pr, head): (&str, i64, &str),
    turn: &str,
) -> Result<(), Error> {
    conn.prepare_cached(
        "UPDATE landings SET turn = ?4 WHERE chat_id = ?1 AND pr = ?2 AND head = ?3",
    )?
    .execute(params![chat, pr, head, turn])?;
    Ok(())
}

/// What a line about PR `pr` of `chat` is about: the task it was built in, as `t:<task>`.
pub fn about_pr(conn: &Connection, chat: &str, pr: i64) -> Result<Option<String>, Error> {
    let task: Option<i64> = conn
        .prepare_cached("SELECT min(number) FROM tasks WHERE chat_id = ?1 AND pr = ?2")?
        .query_row(params![chat, pr], |r| r.get(0))?;
    Ok(task.map(|n| format!("t:{n}")))
}
