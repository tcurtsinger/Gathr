//! The lead's plan for an ask (0062): each call of the `plan` tool writes a version, posted in the
//! chat as the lead's own message with its parts, and checked in the open before the person sees it
//! (0080). A plan's state is read from its rows, never stored apart.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::{self, Brief, Kind, Start};
use crate::team::Role;

pub(crate) mod check;
mod decide;

pub use check::overdue;
pub use decide::{Decided, approve, drop, send_back};

/// Whether ask `?1` has an approved plan.
pub const APPROVED: &str = "EXISTS (SELECT 1 FROM plans p JOIN plan_versions v ON v.plan_id = p.id
        WHERE p.ask_id = ?1 AND v.decided = 'approved')";

/// Version `v` is its plan's newest.
pub(crate) const NEWEST: &str =
    "v.n = (SELECT max(w.n) FROM plan_versions w WHERE w.plan_id = v.plan_id)";

/// The longest title.
const TITLE_MOST: usize = 80;

/// Where a plan stands, read from its newest version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Checking,
    /// On the person.
    Waiting,
    Revising,
    Approved,
    Dropped,
}

/// One part as the lead gives it: a complete task.
#[derive(Default)]
pub struct Part<'a> {
    /// The agent who does it, by name.
    pub to: &'a str,
    pub what: &'a str,
    pub why: Option<&'a str>,
    pub done: Option<&'a str>,
    pub proof: Option<&'a str>,
    pub place: Option<&'a str>,
    pub size: Option<&'a str>,
    pub lines: Option<i64>,
    /// Earlier parts it starts after, by number.
    pub after: &'a [i64],
    /// Tasks of this chat it starts after.
    pub after_tasks: &'a [i64],
}

/// The lead's `plan` call.
pub struct Writing<'a> {
    pub chat: &'a str,
    pub agent: &'a str,
    /// The caller's turn, if it has one.
    pub turn: Option<&'a str>,
    pub title: &'a str,
    pub text: &'a str,
    pub parts: &'a [Part<'a>],
}

/// What writing a version did: its message, for the live socket, and the turns it stops.
#[derive(Debug)]
pub struct Wrote {
    pub plan: i64,
    pub version: i64,
    pub n: i64,
    pub answer: String,
    pub message: Value,
    pub starts: Vec<Start>,
}

/// A part as it is kept.
#[derive(Debug, PartialEq)]
struct Kept {
    owner: String,
    what: String,
    why: String,
    done: String,
    proof: String,
    place: String,
    size: String,
    lines: i64,
    after: Vec<i64>,
    after_tasks: Vec<i64>,
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// Writes the next version of the open plan of the ask the caller's turn serves, or a new plan's
/// first, with its message, in one transaction (H2).
pub fn write(conn: &mut Connection, w: &Writing, trace: &str) -> Result<Wrote, Error> {
    let tx = conn.transaction()?;
    let wrote = write_in(&tx, w, trace)?;
    tx.commit()?;
    Ok(wrote)
}

fn write_in(tx: &Transaction, w: &Writing, trace: &str) -> Result<Wrote, Error> {
    let ask = refuse_caller(tx, w)?;
    let title = w.title.trim();
    if title.is_empty() || title.contains('\n') || title.chars().count() > TITLE_MOST {
        return Err(refused(format!(
            "`title` is one line of 1 to {TITLE_MOST} characters"
        )));
    }
    if w.parts.is_empty() {
        return Err(refused("A plan has at least one part."));
    }
    let parts = kept_parts(tx, w.chat, w.parts)?;
    for (b, part) in parts.iter().enumerate() {
        if let Some(a) = parts[..b].iter().position(|p| p.what == part.what) {
            return Err(refused(format!(
                "Parts {} and {} are both \"{}\": give each part its own what.",
                a + 1,
                b + 1,
                part.what
            )));
        }
    }
    let open = open_version(tx, &ask)?;
    let text = w.text.trim();
    if let Some((_, version, n)) = open {
        let same: (String, String) = tx.query_row(
            "SELECT title, text FROM plan_versions WHERE id = ?1",
            [version],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if same == (title.to_string(), text.to_string()) && read_parts(tx, version)? == parts {
            return Err(refused(format!(
                "Version {} would be the same as version {n}: change a part.",
                n + 1
            )));
        }
    }
    let turn = w.turn.unwrap_or_default();
    let now = now_us();
    let (plan, n) = match open {
        Some((plan, _, n)) => (plan, n + 1),
        None => {
            tx.execute(
                "INSERT INTO plans (chat_id, ask_id, lead, created_ts) VALUES (?1, ?2, ?3, ?4)",
                params![w.chat, ask, w.agent, now],
            )?;
            (tx.last_insert_rowid(), 1)
        }
    };
    let body = body(tx, (title, text, n), &parts)?;
    let message = crate::store::add_message(tx, w.chat, turn, "assistant", "text", &body)?;
    let message_id = message["id"].as_str().unwrap_or_default().to_string();
    tx.execute(
        "INSERT INTO plan_versions (plan_id, n, message_id, turn_id, title, text, created_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![plan, n, message_id, turn, title, text, now],
    )?;
    let version = tx.last_insert_rowid();
    for (i, p) in parts.iter().enumerate() {
        tx.execute(
            "INSERT INTO plan_parts (version_id, n, owner, what, why, done, proof, place, size, size_lines,
                                     after_parts, after_tasks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                version,
                i as i64 + 1,
                p.owner,
                p.what,
                p.why,
                p.done,
                p.proof,
                p.place,
                p.size,
                p.lines,
                json!(p.after).to_string(),
                json!(p.after_tasks).to_string()
            ],
        )?;
    }
    let mut starts = Vec::new();
    if let Some((_, before, _)) = open {
        let why = format!("version {n} replaced it");
        starts.extend(check::cancel_open(tx, before, &why, trace)?);
    }
    Event::point("gathr.plan.version")
        .trace(trace)
        .chat(w.chat)
        .ok()
        .attr("gathr.plan.id", plan)
        .attr("gathr.plan.version", n)
        .attr("gathr.ask.id", ask.as_str())
        .attr("gathr.message.id", message_id.as_str())
        .attr("gathr.plan.parts", parts.len())
        .insert(tx)?;
    // A version the lead's last word writes gets no second round (0080).
    let last_word: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM turns WHERE id = ?1 AND opened_by = 'last_word')",
        [turn],
        |r| r.get(0),
    )?;
    let after = match last_word {
        true => "The person sees it when this turn ends.",
        false => {
            "The members who took no part check it once this turn ends; the person sees it after."
        }
    };
    let answer = format!(
        "Wrote version {n} of plan #{plan}, \"{title}\": {}. {after}",
        counted(parts.len())
    );
    Ok(Wrote {
        plan,
        version,
        n,
        answer,
        message,
        starts,
    })
}

/// H2.4's refusals of who calls and where, in order: the ask the caller's turn serves.
fn refuse_caller(tx: &Connection, w: &Writing) -> Result<String, Error> {
    crate::archive::refuse_archived(tx, w.chat)?;
    let members = crate::team::members(tx, w.chat)?;
    if members.len() < 2 {
        return Err(refused("This chat has one agent, so it has no plans."));
    }
    let lead = members.iter().find(|m| m.role == Role::Supervisor);
    if let Some(lead) = lead.filter(|m| m.agent != w.agent) {
        let name = &lead.name;
        return Err(refused(format!(
            "Only {name}, the chat's Supervisor, writes plans."
        )));
    }
    let turn = w
        .turn
        .ok_or_else(|| refused("you have no turn running in this chat"))?;
    let sql =
        "SELECT u.ask_id, a.size FROM turns u LEFT JOIN asks a ON a.id = u.ask_id WHERE u.id = ?1";
    let found: Option<(Option<String>, Option<String>)> = tx
        .query_row(sql, [turn], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let (ask, size) = found.unwrap_or_default();
    let ask = ask.ok_or_else(|| {
        refused("This turn serves none of the person's asks, so a plan has nothing to belong to.")
    })?;
    if size.as_deref() == Some("repair") {
        return Err(refused(
            "This ask is a repair, which goes ahead without a plan. If it needs one, size it new first.",
        ));
    }
    if let Some(q) = open_question(tx, &ask, 0)? {
        return Err(refused(format!(
            "Question #{q} to the person is open for this ask: write the plan once it's answered."
        )));
    }
    Ok(ask)
}

/// Each part checked as a task is (H2.3), then by the part rules, in order: the parts as kept.
fn kept_parts(conn: &Connection, chat: &str, parts: &[Part]) -> Result<Vec<Kept>, Error> {
    let mut kept = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let n = i as i64 + 1;
        let of_part = |e: Error| match e {
            Error::Refused(why) => refused(format!("Part {n}: {why}")),
            e => e,
        };
        let size = tasks::sized(p.size, p.lines).ok().flatten();
        let b = Brief {
            why: p.why,
            done: p.done,
            proof: p.proof,
            place: p.place,
            size,
        };
        tasks::brief::checked(Kind::Work, p.what, &b).map_err(of_part)?;
        let Some((size, lines)) = size else {
            return Err(refused(format!(
                "Part {n}: a part needs size and lines: the files or area it touches, and the rough lines it adds."
            )));
        };
        let owner = crate::team::agent_named(conn, chat, p.to).map_err(of_part)?;
        if let Some(a) = p.after.iter().find(|a| !(1..n).contains(*a)) {
            return Err(refused(format!(
                "Part {n} comes after part {a}: a part comes after earlier parts only."
            )));
        }
        tasks::edit::in_chat(conn, chat, p.after_tasks).map_err(of_part)?;
        for &t in p.after_tasks {
            if tasks::get(conn, t)?.is_some_and(|t| t.state == tasks::State::Cancelled) {
                return Err(refused(format!(
                    "Part {n} comes after #{t}, which is cancelled."
                )));
            }
        }
        let field = |f: Option<&str>| f.map_or(String::new(), |f| f.trim().to_string());
        kept.push(Kept {
            owner,
            what: p.what.trim().to_string(),
            why: field(p.why),
            done: field(p.done),
            proof: field(p.proof),
            place: field(p.place),
            size: size.trim().to_string(),
            lines,
            after: p.after.to_vec(),
            after_tasks: p.after_tasks.to_vec(),
        });
    }
    Ok(kept)
}

/// "1 part", "3 parts".
fn counted(k: usize) -> String {
    match k {
        1 => "1 part".to_string(),
        k => format!("{k} parts"),
    }
}

/// The version's message: the whole plan, each part in the words its task's brief will use (H2.8).
fn body(
    conn: &Connection,
    (title, text, n): (&str, &str, i64),
    parts: &[Kept],
) -> Result<String, Error> {
    let mut lines = vec![
        format!("Plan: {title} (version {n}, {})", counted(parts.len())),
        text.to_string(),
    ];
    for (i, p) in parts.iter().enumerate() {
        let mut after: Vec<String> = p.after.iter().map(|a| format!("part {a}")).collect();
        after.extend(p.after_tasks.iter().map(|t| format!("#{t}")));
        let after: Vec<&str> = after.iter().map(String::as_str).collect();
        let after = match after.is_empty() {
            true => String::new(),
            false => format!(", after {}", crate::waiting::words::list(&after)),
        };
        let owner = tasks::name_of(conn, &p.owner)?;
        lines.push(String::new());
        lines.push(format!("{}. {} ({owner}){after}", i + 1, p.what));
        lines.push(format!("Why: {}", p.why));
        lines.push(format!("Done when: {}", p.done));
        lines.push(format!("Proof: {}", p.proof));
        lines.push(format!("Where: {}", p.place));
        lines.push(format!("Size: {}, about {} lines", p.size, p.lines));
    }
    Ok(lines.join("\n"))
}

fn read_parts(conn: &Connection, version: i64) -> Result<Vec<Kept>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT owner, what, why, done, proof, place, size, size_lines, after_parts, after_tasks
         FROM plan_parts WHERE version_id = ?1 ORDER BY n",
    )?;
    let list = |r: &rusqlite::Row, i: usize| -> rusqlite::Result<Vec<i64>> {
        let text: String = r.get(i)?;
        Ok(serde_json::from_str(&text).unwrap_or_default())
    };
    let rows = stmt.query_map([version], |r| {
        Ok(Kept {
            owner: r.get(0)?,
            what: r.get(1)?,
            why: r.get(2)?,
            done: r.get(3)?,
            proof: r.get(4)?,
            place: r.get(5)?,
            size: r.get(6)?,
            lines: r.get(7)?,
            after: list(r, 8)?,
            after_tasks: list(r, 9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The ask's open plan, its newest version and that version's number: one whose newest version
/// isn't approved or dropped (H2.5).
fn open_version(conn: &Connection, ask: &str) -> Result<Option<(i64, i64, i64)>, Error> {
    let sql = format!(
        "SELECT p.id, v.id, v.n FROM plans p JOIN plan_versions v ON v.plan_id = p.id
         WHERE p.ask_id = ?1 AND {NEWEST} AND (v.decided IS NULL OR v.decided = 'sent_back')
         ORDER BY p.id DESC LIMIT 1"
    );
    let row = |r: &rusqlite::Row| Ok((r.get(0)?, r.get(1)?, r.get(2)?));
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([ask], row)
        .optional()?)
}

/// A question of the lead's to the person open for the ask, but `except` (H3.1).
fn open_question(conn: &Connection, ask: &str, except: i64) -> Result<Option<i64>, Error> {
    let sql = "SELECT number FROM tasks WHERE ask_id = ?1 AND kind = 'person' AND state IN ('ready', 'held')
                 AND giver IS NOT NULL AND number != ?2
               ORDER BY number LIMIT 1";
    Ok(conn
        .prepare_cached(sql)?
        .query_row(params![ask, except], |r| r.get(0))
        .optional()?)
}

/// Another question open for the ask of the person's task `task`, while the ask may get a plan: it
/// isn't a repair and has no approved plan (H3.1).
pub(crate) fn held_question(conn: &Connection, task: i64) -> Result<Option<i64>, Error> {
    let sql = format!(
        "SELECT t.ask_id FROM tasks t JOIN asks a ON a.id = t.ask_id
         WHERE t.number = ?1 AND a.size IS NOT 'repair' AND NOT {}",
        APPROVED.replace("?1", "t.ask_id")
    );
    let ask: Option<String> = conn.query_row(&sql, [task], |r| r.get(0)).optional()?;
    match ask {
        Some(ask) => open_question(conn, &ask, task),
        None => Ok(None),
    }
}

/// The plan's state (H1.2).
pub fn state(conn: &Connection, plan: i64) -> Result<State, Error> {
    let (decided, sent, left): (Option<String>, Option<i64>, bool) = conn.query_row(
        "SELECT v.decided, v.sent_ts, EXISTS (SELECT 1 FROM tasks t WHERE t.checks = v.id
             AND t.state IN ('blocked', 'cancelled') AND t.finish = 'left')
         FROM plan_versions v WHERE v.plan_id = ?1 ORDER BY v.n DESC LIMIT 1",
        [plan],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(match (decided.as_deref(), sent, left) {
        (Some("approved"), ..) => State::Approved,
        (Some("dropped"), ..) => State::Dropped,
        (Some(_), ..) | (None, None, true) => State::Revising,
        (None, Some(_), _) => State::Waiting,
        _ => State::Checking,
    })
}

/// The state of the ask's open plan: checking, waiting or revising; none with no open plan.
pub fn open_state(conn: &Connection, ask: &str) -> Result<Option<State>, Error> {
    match open_version(conn, ask)? {
        Some((plan, ..)) => Ok(Some(state(conn, plan)?)),
        None => Ok(None),
    }
}

/// The chat's plans whose newest version waits on the person: each lead, title, and since when.
pub fn waiting(conn: &Connection, chat: &str) -> Result<Vec<(String, String, i64)>, Error> {
    let sql = format!(
        "SELECT p.lead, v.title, v.sent_ts FROM plans p JOIN plan_versions v ON v.plan_id = p.id
         WHERE p.chat_id = ?1 AND v.sent_ts IS NOT NULL AND v.decided IS NULL AND {NEWEST}
         ORDER BY v.sent_ts"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// A plan as the chat's tasks list gives it.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub id: i64,
    pub ask: String,
    pub title: String,
    pub lead: String,
    pub state: State,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionSummary {
    pub id: i64,
    pub n: i64,
    pub message: String,
    pub parts: i64,
    pub check_line: Option<String>,
    pub sent: Option<String>,
    pub decided: Option<String>,
    pub decided_line: Option<String>,
}

/// The chat's plans, oldest first.
pub fn in_chat(conn: &Connection, chat: &str) -> Result<Vec<Summary>, Error> {
    let ids: Vec<i64> = {
        let mut stmt =
            conn.prepare_cached("SELECT id FROM plans WHERE chat_id = ?1 ORDER BY id")?;
        let rows = stmt.query_map([chat], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    ids.into_iter()
        .filter_map(|id| summary(conn, id).transpose())
        .collect()
}

fn summary(conn: &Connection, plan: i64) -> Result<Option<Summary>, Error> {
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT ask_id, lead FROM plans WHERE id = ?1",
            [plan],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((ask, lead)) = found else {
        return Ok(None);
    };
    let mut stmt = conn.prepare_cached(
        "SELECT v.id, v.n, v.message_id, (SELECT count(*) FROM plan_parts WHERE version_id = v.id),
                v.check_line, v.sent_ts, v.decided, v.decided_line, v.title
         FROM plan_versions v WHERE v.plan_id = ?1 ORDER BY v.n",
    )?;
    let mut title = String::new();
    let rows = stmt.query_map([plan], |r| {
        let summary = VersionSummary {
            id: r.get(0)?,
            n: r.get(1)?,
            message: r.get(2)?,
            parts: r.get(3)?,
            check_line: r.get(4)?,
            sent: r.get::<_, Option<i64>>(5)?.map(crate::ids::utc),
            decided: r.get(6)?,
            decided_line: r.get(7)?,
        };
        Ok((summary, r.get::<_, String>(8)?))
    })?;
    let mut versions = Vec::new();
    for row in rows {
        let (version, newest) = row?;
        versions.push(version);
        title = newest;
    }
    let state = state(conn, plan)?;
    Ok(Some(Summary {
        id: plan,
        ask,
        title,
        lead,
        state,
        versions,
    }))
}

/// The plan whole, as `GET /api/plans/{plan}` gives it: its summary, the ask's message and its
/// questions, and each version's words, answers, check and parts (H5.7).
pub fn get(conn: &Connection, plan: i64) -> Result<Option<Value>, Error> {
    let Some(summary) = summary(conn, plan)? else {
        return Ok(None);
    };
    let mut shown = serde_json::to_value(&summary)?;
    let asked: Option<String> = conn
        .query_row(
            "SELECT body FROM messages WHERE id = ?1",
            [&summary.ask],
            |r| r.get(0),
        )
        .optional()?;
    shown["asked"] = json!(asked);
    let mut stmt = conn.prepare_cached(
        "SELECT number, what, CASE WHEN state = 'done' THEN reason END FROM tasks
         WHERE ask_id = ?1 AND kind = 'person' ORDER BY number",
    )?;
    let rows = stmt.query_map([&summary.ask], |r| {
        let (number, what, answer): (i64, String, Option<String>) =
            (r.get(0)?, r.get(1)?, r.get(2)?);
        Ok(json!({ "number": number, "what": what, "answer": answer }))
    })?;
    shown["questions"] = Value::Array(rows.collect::<Result<_, _>>()?);
    shown["changed"] = Value::Array(crate::hold::look::of_ask(conn, &summary.ask)?);
    for (i, v) in summary.versions.iter().enumerate() {
        let detail = check::version_detail(conn, v.id)?;
        if let (Some(into), Value::Object(detail)) = (shown["versions"][i].as_object_mut(), detail)
        {
            into.extend(detail);
        }
    }
    Ok(Some(shown))
}
