//! The person's asks in a team chat, and their size (0061). An ask is named by the person's message
//! that started it; each turn and task records the ask it serves (`ask_id`), inherited in code, not
//! by triggers: a migration that rebuilds `turns` or `tasks` would drop a trigger without a word.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::{Start, plan_wait};
use crate::team::Role;

/// How Gathr's line for a repair whose work is in begins.
pub const FIXED: &str = "Fixed:";

/// An ask as the chat's tasks list gives it.
#[derive(Debug, Clone, Serialize)]
pub struct Ask {
    pub id: String,
    pub title: String,
    /// `repair`, `new`, or none yet (which counts as new).
    pub size: Option<String>,
    /// Who sized it last: the lead's agent id, or `person`.
    pub by: Option<String>,
    /// The lead's turn that sized it last.
    pub sized_in: Option<String>,
    /// Gathr refused or caught something of it (0063): an unsized ask's line shows from then.
    pub held: bool,
}

/// An ask has open work while an agent's task of it isn't done or cancelled, a turn of it is
/// queued or running, or its plan is open: checked, waiting on the person or sent back (0062). The
/// person's own questions don't count: Gathr never stale-cancels them.
pub(crate) const OPEN: &str =
    "(EXISTS (SELECT 1 FROM tasks t WHERE t.ask_id = a.id AND t.owner IS NOT NULL
         AND t.state NOT IN ('done', 'cancelled'))
     OR EXISTS (SELECT 1 FROM turns u WHERE u.ask_id = a.id
         AND u.status IN ('queued', 'waiting_sign_in', 'running'))
     OR EXISTS (SELECT 1 FROM plans p JOIN plan_versions v ON v.plan_id = p.id WHERE p.ask_id = a.id
         AND (v.decided IS NULL OR v.decided = 'sent_back')
         AND v.n = (SELECT max(w.n) FROM plan_versions w WHERE w.plan_id = v.plan_id)))";

const ASK: &str = "SELECT a.id, m.body, a.size, a.sized_by, a.sized_in, a.held_ts IS NOT NULL
                   FROM asks a LEFT JOIN messages m ON m.id = a.id";

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

fn in_team(conn: &Connection, chat: &str) -> Result<bool, Error> {
    let sql = "SELECT count(*) >= 2 FROM chat_members WHERE chat_id = ?1";
    Ok(conn.prepare_cached(sql)?.query_row([chat], |r| r.get(0))?)
}

/// The person's `message`, saved in `turn`, starts an ask or joins one: the ask its turn already
/// serves, else the newest with open work, else its own. In the caller's transaction.
pub fn for_message(conn: &Connection, chat: &str, turn: &str, message: &str) -> Result<(), Error> {
    if !in_team(conn, chat)? {
        return Ok(());
    }
    let found: Option<(Option<String>, String)> = conn
        .prepare_cached("SELECT ask_id, trace_id FROM turns WHERE id = ?1")?
        .query_row([turn], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((serves, trace)) = found else {
        return Ok(());
    };
    let (ask, joined) = match serves {
        Some(ask) => (ask, true),
        None => {
            let sql = format!(
                // The newest by when it was saved: two ids of one millisecond don't sort by time.
                "SELECT a.id FROM asks a WHERE a.chat_id = ?1 AND {OPEN} ORDER BY a.rowid DESC LIMIT 1"
            );
            let open: Option<String> = conn
                .prepare_cached(&sql)?
                .query_row([chat], |r| r.get(0))
                .optional()?;
            let joined = open.is_some();
            let ask = match open {
                Some(ask) => ask,
                None => {
                    conn.prepare_cached("INSERT INTO asks (id, chat_id) VALUES (?1, ?2)")?
                        .execute([message, chat])?;
                    message.to_string()
                }
            };
            conn.prepare_cached("UPDATE turns SET ask_id = ?2 WHERE id = ?1 AND ask_id IS NULL")?
                .execute([turn, &ask])?;
            (ask, joined)
        }
    };
    Event::point("gathr.ask.message")
        .trace(&trace)
        .chat(chat)
        .ok()
        .attr("gathr.ask.id", ask)
        .attr("gathr.message.id", message)
        .attr("gathr.turn.id", turn)
        .attr("gathr.ask.joined", joined)
        .insert(conn)?;
    Ok(())
}

/// A new task takes the ask of the turn that gave it, else of its parent.
pub fn inherit(conn: &Connection, task: i64) -> Result<(), Error> {
    conn.prepare_cached(
        "UPDATE tasks SET ask_id = COALESCE((SELECT u.ask_id FROM turns u WHERE u.id = tasks.given_in),
             (SELECT p.ask_id FROM tasks p WHERE p.number = tasks.parent))
         WHERE number = ?1",
    )?
    .execute([task])?;
    Ok(())
}

/// The lead's `size` call.
pub struct Sizing<'a> {
    pub chat: &'a str,
    pub agent: &'a str,
    /// The caller's turn, if it has one.
    pub turn: Option<&'a str>,
    pub size: &'a str,
    /// A message of the person's: the ask it started, or one it joined, to split off.
    pub message: Option<&'a str>,
    pub trace: &'a str,
}

/// Where a message that joined another ask was saved, to split it off.
struct Joined {
    from: String,
    turn: String,
    rowid: i64,
    ts: i64,
}

/// The chat's Supervisor sizes an ask, or makes a message that joined another an ask of its own:
/// what to tell it, and the turns of the tasks its change puts back in line (0063).
pub fn size(conn: &mut Connection, s: &Sizing) -> Result<(String, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let said = size_in(&tx, s)?;
    tx.commit()?;
    Ok(said)
}

fn size_in(tx: &Transaction, s: &Sizing) -> Result<(String, Vec<Start>), Error> {
    if !matches!(s.size, "repair" | "new") {
        return Err(refused(format!(
            "`{}` isn't a size: give repair or new.",
            s.size
        )));
    }
    crate::archive::refuse_archived(tx, s.chat)?;
    let members = crate::team::members(tx, s.chat)?;
    if members.len() < 2 {
        return Err(refused(
            "This chat has one agent, so its asks have no size.",
        ));
    }
    let lead = members.iter().find(|m| m.role == Role::Supervisor);
    if let Some(lead) = lead.filter(|m| m.agent != s.agent) {
        let name = &lead.name;
        return Err(refused(format!(
            "Only {name}, the chat's Supervisor, sizes asks."
        )));
    }
    let turn = s
        .turn
        .ok_or_else(|| refused("you have no turn running in this chat"))?;
    let Some(message) = s.message else {
        let ask: Option<String> = tx
            .query_row("SELECT ask_id FROM turns WHERE id = ?1", [turn], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        let why = "This turn serves none of the person's asks: name the person's message to size its ask.";
        return sized(tx, s, &ask.ok_or_else(|| refused(why))?, turn);
    };
    let found: Option<(Option<String>, i64, i64)> = tx
        .query_row(
            "SELECT turn_id, rowid, ts FROM messages WHERE id = ?1 AND chat_id = ?2 AND role = 'user'",
            [message, s.chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (at, rowid, ts) = found.ok_or_else(|| {
        refused(format!(
            "There's no message {message} of the person's in this chat."
        ))
    })?;
    let own: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM asks WHERE id = ?1)",
        [message],
        |r| r.get(0),
    )?;
    if own {
        return sized(tx, s, message, turn);
    }
    let from: Option<String> = match &at {
        Some(at) => tx.query_row("SELECT ask_id FROM turns WHERE id = ?1", [at], |r| r.get(0))?,
        None => None,
    };
    let (Some(from), Some(at)) = (from, at) else {
        return Err(refused(format!(
            "Message {message} serves no ask: it came before asks, or before this chat had two agents."
        )));
    };
    let joined = Joined {
        from,
        turn: at,
        rowid,
        ts,
    };
    split(tx, s, message, turn, joined)
}

/// The lead sizes an ask: the person's size stands.
fn sized(
    tx: &Transaction,
    s: &Sizing,
    ask: &str,
    turn: &str,
) -> Result<(String, Vec<Start>), Error> {
    let (was, by): (Option<String>, Option<String>) = tx.query_row(
        "SELECT size, sized_by FROM asks WHERE id = ?1",
        [ask],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let title = title(tx, ask)?;
    if was.as_deref() == Some(s.size) {
        return Ok((format!("\"{title}\" is already {}.", s.size), Vec::new()));
    }
    if let (Some("person"), Some(was)) = (by.as_deref(), was.as_deref()) {
        return Err(refused(format!(
            "The person made this ask {was}: only the person changes it now."
        )));
    }
    tx.execute(
        "UPDATE asks SET size = ?2, sized_by = ?3, sized_in = ?4, sized_ts = ?5 WHERE id = ?1",
        params![ask, s.size, s.agent, turn, now_us()],
    )?;
    resized(s.chat, (ask, s.size, was.as_deref()), s.agent, s.trace)
        .attr("gathr.turn.id", turn)
        .insert(tx)?;
    let (_, starts) = plan_wait::resized(tx, ask, s.trace)?;
    Ok((format!("Sized \"{title}\" as {}.", s.size), starts))
}

/// A message that joined another ask becomes one of its own. The turn it opened moves with it,
/// with the tasks that turn gave; a turn it was steered into stays, and only the tasks it gave
/// after the message go.
fn split(
    tx: &Transaction,
    s: &Sizing,
    message: &str,
    turn: &str,
    j: Joined,
) -> Result<(String, Vec<Start>), Error> {
    let was: Option<String> =
        tx.query_row("SELECT size FROM asks WHERE id = ?1", [&j.from], |r| {
            r.get(0)
        })?;
    let opened: bool = tx.query_row(
        "SELECT task IS NULL AND NOT EXISTS (SELECT 1 FROM messages WHERE turn_id = ?1 AND rowid < ?2)
         FROM turns WHERE id = ?1",
        params![j.turn, j.rowid],
        |r| r.get(0),
    )?;
    // A plan, its check and its tasks stay with its ask (0062).
    let wrote: Option<i64> = match opened {
        true => tx
            .query_row(
                "SELECT plan_id FROM plan_versions WHERE turn_id = ?1 LIMIT 1",
                [&j.turn],
                |r| r.get(0),
            )
            .optional()?,
        false => None,
    };
    if let Some(plan) = wrote {
        return Err(refused(format!(
            "Message {message}'s turn wrote plan #{plan}, which belongs to its ask: it can't be split off."
        )));
    }
    tx.execute(
        "INSERT INTO asks (id, chat_id, size, sized_by, sized_in, sized_ts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![message, s.chat, s.size, s.agent, turn, now_us()],
    )?;
    let planned =
        "AND checks IS NULL AND number NOT IN (SELECT task FROM plan_parts WHERE task IS NOT NULL)";
    let tasks = match opened {
        true => {
            tx.execute(
                "UPDATE turns SET ask_id = ?2 WHERE id = ?1",
                [&j.turn, message],
            )?;
            tx.execute(
                &format!("UPDATE tasks SET ask_id = ?2 WHERE given_in = ?1 AND ask_id = ?3 {planned}"),
                [&j.turn, message, &j.from],
            )?
        }
        false => tx.execute(
            &format!(
                "UPDATE tasks SET ask_id = ?2 WHERE given_in = ?1 AND created_ts >= ?4 AND ask_id = ?3 {planned}"
            ),
            params![j.turn, message, j.from, j.ts],
        )?,
    };
    // A moved task's own turns (its brief, a report ask, a carry-on) serve its ask, as G1.2 has it.
    tx.execute(
        "UPDATE turns SET ask_id = ?1 WHERE task IN (SELECT number FROM tasks WHERE ask_id = ?1)",
        [message],
    )?;
    resized(s.chat, (message, s.size, was.as_deref()), s.agent, s.trace)
        .attr("gathr.ask.split", true)
        .attr("gathr.ask.from", j.from.as_str())
        .attr("gathr.turn.moved", opened)
        .attr("gathr.task.count", tasks)
        .attr("gathr.turn.id", turn)
        .insert(tx)?;
    let (_, starts) = plan_wait::resized(tx, message, s.trace)?;
    let title = title(tx, message)?;
    let said = format!(
        "Made message {message} an ask of its own and sized \"{title}\" as {}.",
        s.size
    );
    Ok((said, starts))
}

/// `gathr.ask.resized`. Only the person's change from repair to new is a correction.
fn resized(
    chat: &str,
    (ask, size, was): (&str, &str, Option<&str>),
    by: &str,
    trace: &str,
) -> Event {
    let correction = by == "person" && was == Some("repair") && size == "new";
    Event::point("gathr.ask.resized")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("gathr.ask.id", ask)
        .attr("gathr.ask.size", size)
        .attr("gathr.ask.was", was)
        .attr("gathr.ask.by", by)
        .attr("gathr.ask.correction", correction)
}

/// The person changes an ask's size: the ask as the chat's tasks list has it, and the turns that
/// follow: its tasks back in line, and its agent told (0063).
pub fn resize(
    conn: &mut Connection,
    ask: &str,
    size: &str,
    trace: &str,
) -> Result<(Ask, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let found: Option<(String, Option<String>)> = tx
        .query_row("SELECT chat_id, size FROM asks WHERE id = ?1", [ask], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let (chat, was) = found.ok_or_else(|| refused(format!("there's no ask {ask}")))?;
    if !matches!(size, "repair" | "new") {
        return Err(refused(format!("`{size}` isn't a size: repair or new")));
    }
    crate::archive::refuse_archived(&tx, &chat)?;
    let mut starts = Vec::new();
    if was.as_deref() != Some(size) {
        tx.execute(
            "UPDATE asks SET size = ?2, sized_by = 'person', sized_ts = ?3 WHERE id = ?1",
            params![ask, size, now_us()],
        )?;
        resized(&chat, (ask, size, was.as_deref()), "person", trace).insert(&tx)?;
        let (moved, released) = plan_wait::resized(&tx, ask, trace)?;
        plan_wait::noted(&tx, ask, &moved)?;
        starts.extend(released);
        starts.extend(plan_wait::told(&tx, ask, (size, was.as_deref()), &moved)?);
    }
    let ask = read(&tx, ask)?;
    tx.commit()?;
    Ok((ask, starts))
}

/// Says "Fixed" once for each repair of the chat whose work is in: a work task of it done, every
/// task of it done or cancelled, and nothing of it queued or running, nor answering a turn of it.
pub fn fixed(conn: &mut Connection, chat: &str, trace: &str) -> Result<(), Error> {
    let tx = conn.transaction()?;
    let due: Vec<String> = {
        let mut stmt = tx.prepare_cached(
            "SELECT a.id FROM asks a WHERE a.chat_id = ?1 AND a.size = 'repair' AND a.fixed IS NULL
             AND EXISTS (SELECT 1 FROM tasks t WHERE t.ask_id = a.id AND t.kind = 'work' AND t.state = 'done')
             AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.ask_id = a.id AND t.state NOT IN ('done', 'cancelled'))
             AND NOT EXISTS (SELECT 1 FROM turns u LEFT JOIN turns p ON p.id = u.answers
                 WHERE u.chat_id = a.chat_id AND u.status IN ('queued', 'waiting_sign_in', 'running')
                 AND (u.ask_id = a.id OR p.ask_id = a.id))
             ORDER BY a.rowid",
        )?;
        let rows = stmt.query_map([chat], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for ask in due {
        let done: Vec<i64> = {
            let mut stmt = tx.prepare_cached(
                "SELECT number FROM tasks WHERE ask_id = ?1 AND kind = 'work' AND state = 'done' ORDER BY number",
            )?;
            let rows = stmt.query_map([&ask], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut lines = vec![format!("{FIXED} {}", first_line(&tx, &ask)?)];
        for number in &done {
            if let Some(t) = crate::tasks::get(&tx, *number)? {
                lines.push(line(&t));
            }
        }
        let said = crate::store::say(&tx, chat, &lines.join("\n"))?;
        let id = said["id"].as_str().unwrap_or_default();
        tx.execute("UPDATE asks SET fixed = ?2 WHERE id = ?1", [&ask, id])?;
        Event::point("gathr.ask.fixed")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("gathr.ask.id", ask.as_str())
            .attr("gathr.message.id", id)
            .attr("gathr.task.count", done.len())
            .insert(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

/// A done task in the "Fixed" line: its PR, else what its report names, and its proof.
fn line(t: &crate::tasks::Task) -> String {
    let mut line = format!("#{} {}", t.number, t.what);
    match (t.pr, t.report.as_ref().and_then(|r| r.see.as_deref())) {
        (Some(pr), _) => line.push_str(&format!(": PR #{pr}")),
        (None, Some(see)) => line.push_str(&format!(": {see}")),
        (None, None) => {}
    }
    if let Some(proof) = &t.proof {
        line.push_str(&format!(". Proof: {proof}"));
    }
    line
}

/// The ask's title: the first line of the message that started it, as a chat's title is made.
pub(crate) fn title(conn: &Connection, ask: &str) -> Result<String, Error> {
    Ok(body(conn, ask)?
        .and_then(|b| crate::store::title_from(&b))
        .unwrap_or_default())
}

/// The first line of the message that started the ask, whole: "Fixed" names the ask by it.
fn first_line(conn: &Connection, ask: &str) -> Result<String, Error> {
    let body = body(conn, ask)?;
    Ok(body
        .as_deref()
        .and_then(crate::text::first_line)
        .unwrap_or_default()
        .to_string())
}

fn body(conn: &Connection, ask: &str) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row("SELECT body FROM messages WHERE id = ?1", [ask], |r| {
            r.get(0)
        })
        .optional()?)
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Ask> {
    let body: Option<String> = r.get(1)?;
    Ok(Ask {
        id: r.get(0)?,
        title: body
            .and_then(|b| crate::store::title_from(&b))
            .unwrap_or_default(),
        size: r.get(2)?,
        by: r.get(3)?,
        sized_in: r.get(4)?,
        held: r.get(5)?,
    })
}

fn read(conn: &Connection, ask: &str) -> Result<Ask, Error> {
    let sql = format!("{ASK} WHERE a.id = ?1");
    Ok(conn.prepare_cached(&sql)?.query_row([ask], row)?)
}

/// The chat's asks, oldest first.
pub fn in_chat(conn: &Connection, chat: &str) -> Result<Vec<Ask>, Error> {
    let sql = format!("{ASK} WHERE a.chat_id = ?1 ORDER BY a.rowid");
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([chat], row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}
