//! A version's check. Since 0080 the members check a version in the open (`crate::open_check`),
//! and it reaches the person when that round ends. What is left here: the 10-minute cap, and what an
//! old chat needs, whose versions were checked by one checker's review task (0062).

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::NEWEST;
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::notes;
use crate::tasks::{self, Finish, Start, State};

/// Cancels an old check task of the version (0062) unless it is done or cancelled.
pub(crate) fn cancel_open(
    tx: &Transaction,
    version: i64,
    why: &str,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let open: Vec<i64> = {
        let mut stmt = tx.prepare_cached(
            "SELECT number FROM tasks WHERE checks = ?1 AND state NOT IN ('done', 'cancelled')",
        )?;
        let rows = stmt.query_map([version], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut starts = Vec::new();
    for task in open {
        starts.extend(tasks::cancel_in(tx, task, why, trace)?);
    }
    Ok(starts)
}

/// Ten minutes with no end to its check (H4.12): each newest undecided version of the chat not sent
/// yet, in a chat not archived, written at least `after_us` before `now`, reaches the person with
/// Gathr's line naming who is still checking it. How many did.
pub fn overdue(
    conn: &mut Connection,
    chat: &str,
    now: i64,
    after_us: i64,
    trace: &str,
) -> Result<usize, Error> {
    let tx = conn.transaction()?;
    let sql = format!(
        "SELECT v.id, v.plan_id, v.n, v.title, p.ask_id FROM plan_versions v
         JOIN plans p ON p.id = v.plan_id JOIN chats c ON c.id = p.chat_id
         WHERE p.chat_id = ?1 AND v.decided IS NULL AND v.sent_ts IS NULL AND c.archived_ts IS NULL
           AND {NEWEST} AND v.created_ts <= ?2
         ORDER BY v.id"
    );
    type Row = (i64, i64, i64, String, String);
    let due: Vec<Row> = {
        let mut stmt = tx.prepare_cached(&sql)?;
        let row = |r: &rusqlite::Row| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?));
        let rows = stmt.query_map(params![chat, now - after_us], row)?;
        rows.collect::<Result<_, _>>()?
    };
    for (version, plan, n, title, ask) in &due {
        let who = still_checking(&tx, chat, ask, *version)?;
        let line = format!("{who} didn't finish checking version {n} of the plan \"{title}\".");
        let said = crate::store::say(&tx, chat, &line)?;
        tx.execute(
            "UPDATE plan_versions SET check_line = ?2, sent_ts = ?3 WHERE id = ?1",
            params![version, said["id"].as_str(), now_us()],
        )?;
        Event::point("gathr.plan.checked")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("gathr.plan.id", *plan)
            .attr("gathr.plan.version", *n)
            .attr("gathr.plan.outcome", "late")
            .attr("gathr.plan.sent", true)
            .insert(&tx)?;
    }
    tx.commit()?;
    Ok(due.len())
}

/// Who is still at the ask's check, by name: its members' check turns and the lead's last word, and
/// an old check task of the version; "The team" when nobody's turn has begun.
fn still_checking(conn: &Connection, chat: &str, ask: &str, version: i64) -> Result<String, Error> {
    let sql = format!(
        "SELECT a.name FROM turns u JOIN agents a ON a.id = u.agent_id
         WHERE u.chat_id = ?1 AND u.ask_id = ?2 AND u.opened_by IN ('check', 'last_word')
           AND u.status IN ('queued', 'waiting_sign_in', 'running')
         UNION ALL
         SELECT a.name FROM tasks t JOIN agents a ON a.id = t.owner
         WHERE t.checks = ?3 AND t.state NOT IN {}",
        tasks::SETTLED
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![chat, ask, version], |r| r.get::<_, String>(0))?;
    let mut names: Vec<String> = Vec::new();
    for name in rows {
        let name = name?;
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.sort();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    Ok(match names.is_empty() {
        true => "The team".to_string(),
        false => crate::waiting::words::list(&names),
    })
}

/// A version's words, answers, check and parts, as `GET /api/plans/{plan}` adds them (H5.7). The
/// answers and the check are an old chat's (0062): a version checked in the open has neither.
pub(super) fn version_detail(conn: &Connection, version: i64) -> Result<Value, Error> {
    let (title, text, created, words, answers): (
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT title, text, created_ts, words, answers FROM plan_versions WHERE id = ?1",
        [version],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    let answers: Value = answers
        .and_then(|a| serde_json::from_str(&a).ok())
        .unwrap_or(json!([]));
    let check: Option<i64> = conn
        .query_row(
            "SELECT number FROM tasks WHERE checks = ?1 ORDER BY number DESC LIMIT 1",
            [version],
            |r| r.get(0),
        )
        .optional()?;
    let check = match check.map(|n| tasks::get(conn, n)).transpose()?.flatten() {
        Some(t) => {
            let sent: bool = conn.query_row(
                "SELECT sent_ts IS NOT NULL FROM plan_versions WHERE id = ?1",
                [version],
                |r| r.get(0),
            )?;
            let left = t.report.as_ref().is_some_and(|r| r.how == Finish::Left);
            // Findings stand once said, though a newer version or the person cancels the check
            // after. A left still waiting on its turn when cancelled was never said: it was sent on.
            let outcome = match t.state {
                State::Done => Some("agrees"),
                State::Blocked if left => Some("findings"),
                State::Cancelled if left && !sent => Some("findings"),
                s if s.settled() => Some("unfinished"),
                _ if sent => Some("late"),
                _ => None,
            };
            let notes: Vec<Value> = notes::newest(conn, t.number, false)?
                .into_iter()
                .map(|(id, n)| json!({ "id": id, "kind": n.kind, "file": n.file, "says": n.says }))
                .collect();
            json!({ "task": t.number, "by": t.owner, "state": t.state, "outcome": outcome, "notes": notes })
        }
        None => Value::Null,
    };
    let mut stmt = conn.prepare_cached(
        "SELECT n, owner, what, why, done, proof, place, size, size_lines, after_parts, after_tasks, task
         FROM plan_parts WHERE version_id = ?1 ORDER BY n",
    )?;
    let rows = stmt.query_map([version], |r| {
        let list = |i: usize| -> rusqlite::Result<Value> {
            Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or(json!([])))
        };
        Ok(json!({
            "n": r.get::<_, i64>(0)?, "owner": r.get::<_, String>(1)?, "what": r.get::<_, String>(2)?,
            "why": r.get::<_, String>(3)?, "done": r.get::<_, String>(4)?, "proof": r.get::<_, String>(5)?,
            "where": r.get::<_, String>(6)?, "size": r.get::<_, String>(7)?, "lines": r.get::<_, i64>(8)?,
            "after": list(9)?, "after_tasks": list(10)?, "task": r.get::<_, Option<i64>>(11)?,
        }))
    })?;
    let parts: Vec<Value> = rows.collect::<Result<_, _>>()?;
    Ok(json!({
        "title": title, "text": text, "created": crate::ids::utc(created), "words": words,
        "answers": answers, "check": check, "parts": parts,
    }))
}
