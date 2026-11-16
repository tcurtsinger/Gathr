//! A version's check (0062, H4): a review task with `checks` set, given to the chat's Reviewer or
//! the free member that has checked least. A thumbs-up on the version agrees; a left report with a
//! wrong or unasked note objects. When it ends, or 10 minutes after the version, the version
//! reaches the person, but for findings, which go back to the lead.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::{Answer, NEWEST};
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::notes::{self, Note};
use crate::tasks::{self, Brief, Finish, Give, Kind, Said, Start, State, Task, Took};
use crate::team::Role;

/// A check that left findings: its task, its owner and its wrong and unasked notes.
pub(crate) struct Left {
    pub check: i64,
    pub owner: String,
    pub notes: Vec<(i64, Note)>,
}

/// The findings of the version's check, while the version is undecided: its newest check left.
pub(crate) fn left_on(conn: &Connection, version: i64) -> Result<Option<Left>, Error> {
    let found: Option<(i64, String)> = conn
        .query_row(
            "SELECT t.number, t.owner FROM tasks t JOIN plan_versions v ON v.id = t.checks
             WHERE v.id = ?1 AND v.decided IS NULL AND t.finish = 'left' AND t.owner IS NOT NULL
             ORDER BY t.number DESC LIMIT 1",
            [version],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((check, owner)) = found else {
        return Ok(None);
    };
    let notes = notes::newest(conn, check, true)?;
    Ok((!notes.is_empty()).then_some(Left {
        check,
        owner,
        notes,
    }))
}

/// Who checks (H4.2): who left findings on the version before, while in the team and not the lead;
/// else the chat's Reviewer, busy or not; else the free member that has checked least.
fn checker(tx: &Connection, chat: &str, lead: &str, left: Option<&Left>) -> Result<String, Error> {
    let members = crate::team::members(tx, chat)?;
    if let Some(left) =
        left.filter(|l| l.owner != lead && members.iter().any(|m| m.agent == l.owner))
    {
        return Ok(left.owner.clone());
    }
    if let Some(reviewer) = members.iter().find(|m| m.role == Role::Reviewer) {
        return Ok(reviewer.agent.clone());
    }
    let sql = "SELECT m.agent_id FROM chat_members m WHERE m.chat_id = ?1 AND m.role != 'supervisor'
               ORDER BY EXISTS (SELECT 1 FROM turns u WHERE u.agent_id = m.agent_id
                                AND u.status IN ('queued', 'waiting_sign_in', 'running')),
                        (SELECT count(*) FROM tasks t WHERE t.chat_id = m.chat_id AND t.owner = m.agent_id
                         AND t.checks IS NOT NULL),
                        m.added_ts, m.rowid
               LIMIT 1";
    Ok(tx.query_row(sql, [chat], |r| r.get(0))?)
}

/// Gives the version's check in the version's transaction (H4.1): its number, its checker's name
/// and its turn, whose brief is the check's own before anything reads it.
pub(super) fn give(
    tx: &Transaction,
    (chat, lead, turn): (&str, &str, &str),
    (version, n, title): (i64, i64, &str),
    left: Option<&Left>,
    trace: &str,
) -> Result<(i64, String, Vec<Start>), Error> {
    let owner = checker(tx, chat, lead, left)?;
    let what = crate::text::cut(
        &format!("Check plan v{n}: {title}"),
        tasks::TITLE_MOST,
        false,
    );
    let g = Give {
        chat,
        giver: Some(lead),
        owner: &owner,
        given_in: Some(turn),
        parent: None,
        kind: Kind::Review,
        what: &what,
        brief: Brief::default(),
        after: &[],
        pr: None,
        in_passing: true,
        look_into: false,
    };
    let (task, mut starts) = tasks::give_in(tx, &g, trace)?;
    tx.execute(
        "UPDATE tasks SET checks = ?2 WHERE number = ?1",
        [task.number, version],
    )?;
    if let Some((_, message)) = tasks::edit::rewritten(tx, task.number)? {
        for start in starts.iter_mut().filter(|s| s.task == Some(task.number)) {
            start.text = message["body"].as_str().unwrap_or_default().to_string();
            start.message = message.clone();
        }
    }
    Ok((task.number, tasks::name_of(tx, &owner)?, starts))
}

/// Cancels the version's check unless it is done or cancelled.
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

/// Only Gathr's own rules and the person end a check, and nobody restarts one (H4.3).
pub(crate) fn refuse_change(
    conn: &Connection,
    task: i64,
    version: i64,
    by: Option<&str>,
    what: &str,
) -> Result<(), Error> {
    if what == "restart" {
        return Err(Error::Refused(format!(
            "Task #{task} is a plan check: a new version gets a new check."
        )));
    }
    if by.is_none() {
        return Ok(());
    }
    let plan: i64 = conn.query_row(
        "SELECT plan_id FROM plan_versions WHERE id = ?1",
        [version],
        |r| r.get(0),
    )?;
    Err(Error::Refused(format!(
        "Task #{task} is plan #{plan}'s check: Gathr ends it when a newer version is written or the plan is decided."
    )))
}

/// A check's brief (H4.6): the first form, or after findings the again form, which lists only
/// those findings with the lead's answers; with D's carry line when it is readied again.
pub(crate) fn check_brief(
    conn: &Connection,
    t: &Task,
    giver: &str,
    carry: Option<&str>,
) -> Result<String, Error> {
    let version = t.checks.unwrap_or_default();
    type Row = (i64, i64, String, String, Option<String>, String, String);
    let (plan, n, title, message, answers, body, asked): Row = conn.query_row(
        "SELECT v.plan_id, v.n, v.title, v.message_id, v.answers,
                (SELECT body FROM messages WHERE id = v.message_id),
                coalesce((SELECT body FROM messages WHERE id = p.ask_id), '')
         FROM plan_versions v JOIN plans p ON p.id = v.plan_id WHERE v.id = ?1",
        [version],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        },
    )?;
    let before: Option<i64> = conn
        .query_row(
            "SELECT id FROM plan_versions WHERE plan_id = ?1 AND n = ?2",
            [plan, n - 1],
            |r| r.get(0),
        )
        .optional()?;
    let left = match before {
        Some(before) => left_on(conn, before)?,
        None => None,
    };
    let left = left.filter(|l| t.owner.as_deref() == Some(l.owner.as_str()));
    let mut lines = vec![format!("Task #{} from {giver}: {}", t.number, t.what)];
    let last = match &left {
        None => {
            lines.push(format!(
                "Check version {n} of {giver}'s plan \"{title}\" before the person sees it. Say what is wrong or \
                 missing (a wrong fact or file, a missed step or caller, a part given to the wrong member, a proof \
                 that proves nothing) and what the ask doesn't need, with file and line."
            ));
            lines.push(format!("The person asked: {asked}"));
            format!(
                "If you agree, give message {message} a thumbs-up with react: that finishes this task. Else \
                 finish_task left, with notes: wrong, unasked or suggestion, each with its evidence in says."
            )
        }
        Some(left) => {
            lines.push(format!(
                "Check version {n} of {giver}'s plan \"{title}\" for your findings on version {} only, with \
                 {giver}'s answers:",
                n - 1
            ));
            let answers: Vec<Answer> = answers
                .and_then(|a| serde_json::from_str(&a).ok())
                .unwrap_or_default();
            for (id, note) in &left.notes {
                let file = note
                    .file
                    .as_ref()
                    .map_or(String::new(), |f| format!("{f}: "));
                let answer = answers
                    .iter()
                    .find(|a| a.note == *id)
                    .map_or(String::new(), |a| {
                        let how = if a.changed { "changed" } else { "kept" };
                        format!(" {giver}: {how}: {}", a.why)
                    });
                let (kind, says) = (note.kind.as_str(), tasks::words::sentence(&note.says));
                lines.push(format!("- Note {id}, {kind}: {file}{says}{answer}"));
            }
            format!(
                "If the answers hold, give message {message} a thumbs-up with react: that finishes this task. \
                 Else finish_task left, with notes on what still fails."
            )
        }
    };
    lines.push(format!("The plan, message {message}:"));
    lines.push(body);
    if let Some(carry) = carry {
        lines.push(tasks::words::carry_line(carry));
    }
    lines.push(last);
    Ok(lines.join("\n"))
}

/// What a check that left findings says of them, for its reason (H4.7).
pub(crate) fn findings(conn: &Connection, task: i64) -> Result<String, Error> {
    let each: Vec<String> = notes::newest(conn, task, true)?
        .iter()
        .map(|(id, n)| {
            // The notes are joined into one sentence, which ends in Gathr's own full stop.
            let says = n.says.trim_end().trim_end_matches('.');
            match &n.file {
                Some(file) => format!("note {id}, {}, {file}: {says}", n.kind.as_str()),
                None => format!("note {id}, {}: {says}", n.kind.as_str()),
            }
        })
        .collect();
    Ok(format!(
        "Findings: {}. Answer each in the next version's answers.",
        each.join("; ")
    ))
}

/// What a thumbs-up on a version's message did: its checker's check finished, as the report took it.
pub struct Agreed {
    pub took: Took,
    pub check: i64,
    /// The version's number.
    pub version: i64,
    pub starts: Vec<Start>,
}

/// After `agent`'s new thumbs-up on `message` (H4.9): when it is a version's message and `agent`
/// owns that version's open check, the check is reported done.
pub fn agreed(
    tx: &Transaction,
    chat: &str,
    message: &str,
    agent: &str,
    trace: &str,
) -> Result<Option<Agreed>, Error> {
    let found: Option<(i64, i64)> = tx
        .query_row(
            "SELECT t.number, v.n FROM plan_versions v JOIN plans p ON p.id = v.plan_id
             JOIN tasks t ON t.checks = v.id
             WHERE v.message_id = ?1 AND p.chat_id = ?2 AND t.owner = ?3 AND t.state NOT IN ('done', 'cancelled')
             ORDER BY t.number DESC LIMIT 1",
            [message, chat, agent],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((check, version)) = found else {
        return Ok(None);
    };
    let said = Said {
        nothing_unasked: true,
        ..Said::default()
    };
    let (_, took, starts, _) = tasks::finish_in(tx, agent, check, Finish::Done, &said, trace)?;
    Ok(Some(Agreed {
        took,
        check,
        version,
        starts,
    }))
}

/// A check settled (H4.11): for the newest undecided version with no check line, in a chat not
/// archived, Gathr says how it ended, and the version reaches the person but for findings.
pub(crate) fn ended(conn: &Connection, task: i64, state: State, trace: &str) -> Result<(), Error> {
    let sql = format!(
        "SELECT v.id, v.plan_id, v.n, v.title, p.chat_id, t.owner, t.finish FROM tasks t
         JOIN plan_versions v ON v.id = t.checks JOIN plans p ON p.id = v.plan_id
         JOIN chats c ON c.id = p.chat_id
         WHERE t.number = ?1 AND v.decided IS NULL AND v.check_line IS NULL AND c.archived_ts IS NULL
           AND {NEWEST}"
    );
    type Row = (
        i64,
        i64,
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
    );
    let row = |r: &rusqlite::Row| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
        ))
    };
    let found: Option<Row> = conn
        .prepare_cached(&sql)?
        .query_row([task], row)
        .optional()?;
    let Some((version, plan, n, title, chat, owner, finish)) = found else {
        return Ok(());
    };
    let checker = tasks::owner_name(conn, owner.as_deref())?;
    let on = format!("version {n} of the plan \"{title}\"");
    let (outcome, line) = match (state, finish.as_deref()) {
        (State::Done, _) => ("agrees", format!("{checker} agrees with {on}.")),
        (State::Blocked, Some("left")) => {
            let found = match notes::newest(conn, task, true)?.len() {
                1 => "1 finding".to_string(),
                k => format!("{k} findings"),
            };
            ("findings", format!("{checker} has {found} on {on}."))
        }
        _ => (
            "unfinished",
            format!("{checker} didn't finish checking {on}."),
        ),
    };
    told(
        conn,
        (&chat, plan, version, n),
        (task, outcome, &line),
        trace,
    )
}

/// Says the check's line in no turn, records it on the version with when it reached the person
/// (never for findings), and its event.
fn told(
    conn: &Connection,
    (chat, plan, version, n): (&str, i64, i64, i64),
    (task, outcome, line): (i64, &str, &str),
    trace: &str,
) -> Result<(), Error> {
    let said = crate::store::say(conn, chat, line)?;
    let sent = outcome != "findings";
    conn.execute(
        "UPDATE plan_versions SET check_line = ?2, sent_ts = CASE WHEN ?3 THEN ?4 ELSE sent_ts END WHERE id = ?1",
        params![version, said["id"].as_str(), sent, now_us()],
    )?;
    Event::point("gathr.plan.checked")
        .trace(trace)
        .chat(chat)
        .task(task)
        .ok()
        .attr("gathr.plan.id", plan)
        .attr("gathr.plan.version", n)
        .attr("gathr.task.number", task)
        .attr("gathr.plan.outcome", outcome)
        .attr("gathr.plan.sent", sent)
        .insert(conn)?;
    Ok(())
}

/// Ten minutes with no answer (H4.12): each newest undecided version of the chat not sent yet,
/// whose check isn't settled, written at least `after_us` before `now`, reaches the person. How
/// many did.
pub fn overdue(
    conn: &mut Connection,
    chat: &str,
    now: i64,
    after_us: i64,
    trace: &str,
) -> Result<usize, Error> {
    let tx = conn.transaction()?;
    let sql = format!(
        "SELECT v.id, v.plan_id, v.n, v.title, t.number, t.owner FROM plan_versions v
         JOIN plans p ON p.id = v.plan_id JOIN tasks t ON t.checks = v.id
         WHERE p.chat_id = ?1 AND v.decided IS NULL AND v.sent_ts IS NULL
           AND {NEWEST} AND t.state NOT IN {} AND v.created_ts <= ?2
         ORDER BY v.id",
        tasks::SETTLED
    );
    type Row = (i64, i64, i64, String, i64, Option<String>);
    let due: Vec<Row> = {
        let mut stmt = tx.prepare_cached(&sql)?;
        let row = |r: &rusqlite::Row| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        };
        let rows = stmt.query_map(params![chat, now - after_us], row)?;
        rows.collect::<Result<_, _>>()?
    };
    for (version, plan, n, title, task, owner) in &due {
        let checker = tasks::owner_name(&tx, owner.as_deref())?;
        let line = format!("{checker} didn't finish checking version {n} of the plan \"{title}\".");
        told(
            &tx,
            (chat, *plan, *version, *n),
            (*task, "late", &line),
            trace,
        )?;
    }
    tx.commit()?;
    Ok(due.len())
}

/// A version's words, answers, check and parts, as `GET /api/plans/{plan}` adds them (H5.7).
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
            let outcome = match t.state {
                State::Done => Some("agrees"),
                State::Blocked if left => Some("findings"),
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
