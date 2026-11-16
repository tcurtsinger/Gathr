//! A question and the member's request it was filed for (0065): the answer settles the request in
//! its own transaction, a folded request waits on the question quietly, and a Discuss takes the
//! question off the person's list until the lead's turn that hears it ends, unless a new question
//! in that turn takes its place.

use rusqlite::{Connection, OptionalExtension, Params, Transaction, params};
use serde::Deserialize;
use serde_json::json;

use super::super::records::heard_as;
use super::super::{
    Finish, Kind, Start, State, Task, cascade, end_waits, read, refused, set, words,
};
use super::{Form, picked};
use crate::Error;
use crate::event::Event;

/// The person's answer as `answer_one` keeps it.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Kept {
    pick: Vec<String>,
    yes: Option<bool>,
    done: bool,
    text: Option<String>,
    by: Option<String>,
}

/// The answer as agents hear it, the pick and the words apart: "Picked: Short. Their words: …".
pub(crate) fn answer_words(conn: &Connection, t: &Task) -> Result<String, Error> {
    let a: Kept = t
        .answer
        .clone()
        .and_then(|a| serde_json::from_value(a).ok())
        .unwrap_or_default();
    let pick = picked(t.form.unwrap_or(Form::Done), &a.pick, a.yes, a.done);
    let text = a.text.as_deref().map(str::trim).filter(|w| !w.is_empty());
    let said: Vec<String> = [
        pick.map(|p| format!("Picked: {p}")),
        text.map(|w| format!("Their words: {w}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut said = said.join(". ");
    if let Some(agent) = a.by {
        said.push_str(&format!(
            " (answered by {} for the person)",
            words::name_of(conn, &agent)?
        ));
    }
    Ok(said)
}

/// The answered question `q`'s request, when it has one still open, is done with the answer: its
/// waits end, and its member hears.
pub(crate) fn settle_request(tx: &Transaction, q: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let t = read(tx, q)?;
    let Some(r) = t.request else {
        return Ok(Vec::new());
    };
    let request = read(tx, r)?;
    if matches!(request.state, State::Done | State::Cancelled) {
        return Ok(Vec::new());
    }
    let why = format!(
        "the person answered #{q} ({}): {}",
        t.what,
        answer_words(tx, &t)?
    );
    set(tx, r, State::Done, Some(&why), trace)?;
    end_waits(tx, r, "its request was answered")?;
    let starts = cascade(tx, r, trace)?;
    Event::point("gathr.task.request")
        .trace(trace)
        .chat(&t.chat)
        .task(r)
        .ok()
        .attr("gathr.task.question", q)
        .attr("gen_ai.agent.id", request.giver)
        .attr("gathr.request.how", "answered")
        .insert(tx)?;
    Ok(starts)
}

/// `blocked` on a request with its question open is refused: the answer reaches its member by
/// itself, and blocking it would tell the member now and again later.
pub(crate) fn refuse_blocked(conn: &Connection, t: &Task, how: Finish) -> Result<(), Error> {
    if how != Finish::Blocked || t.kind != Kind::PersonRequest {
        return Ok(());
    }
    let sql = "SELECT number FROM tasks WHERE kind = 'person' AND request = ?1 AND state IN ('ready', 'held')";
    let Some(q) = conn
        .query_row(sql, [t.number], |r| r.get::<_, i64>(0))
        .optional()?
    else {
        return Ok(());
    };
    let member = words::owner_name(conn, t.giver.as_deref())?;
    Err(refused(format!(
        "#{} waits on #{q}, the person's question, and its answer reaches {member} by itself. End your turn with \
         no finish_task.",
        t.number
    )))
}

/// A request folded onto a question still open: it waits for that answer, and its member isn't
/// told it's blocked.
pub(crate) fn folded(conn: &Connection, t: &Task) -> Result<bool, Error> {
    let (Kind::PersonRequest, State::Blocked, Some(on)) = (t.kind, t.state, t.blocked_on) else {
        return Ok(false);
    };
    let sql = "SELECT EXISTS (SELECT 1 FROM tasks WHERE number = ?1 AND kind = 'person' AND state IN ('ready', 'held'))";
    Ok(conn.query_row(sql, [on], |r| r.get(0))?)
}

/// What a task blocked on `task`, now done, carries on from: with the answer when `task` is a
/// question or a request. A member's own task blocked on its request hears it there, so the
/// request counts as heard. A request folded on the question is the lead's to finish with the
/// answer: its member hears nothing until then.
pub(crate) fn carry(tx: &Transaction, task: i64, blocked: i64) -> Result<String, Error> {
    let t = read(tx, task)?;
    let answer = match t.kind {
        Kind::Person => Some(answer_words(tx, &t)?),
        Kind::PersonRequest => {
            tx.execute(
                "UPDATE tasks SET heard = 1, heard_as = ?2
                 WHERE number = ?1 AND giver = (SELECT owner FROM tasks WHERE number = ?3)",
                params![task, heard_as(State::Done, t.reason.as_deref()), blocked],
            )?;
            t.reason
        }
        _ => None,
    };
    let carry = format!("the task blocking this one, #{task}, is done");
    let Some(answer) = answer else {
        return Ok(carry);
    };
    let carry = format!("{carry}: {answer}");
    let b = read(tx, blocked)?;
    if t.kind != Kind::Person || b.kind != Kind::PersonRequest {
        return Ok(carry);
    }
    let member = words::owner_name(tx, b.giver.as_deref())?;
    Ok(format!(
        "{} Finish #{blocked} done with this answer in note: {member} hears it then.",
        words::sentence(&carry)
    ))
}

fn discussed(t: &Task, how: &str, trace: &str) -> Event {
    let event = Event::point("gathr.task.discussed")
        .trace(trace)
        .chat(&t.chat);
    event.task(t.number).ok().attr("gathr.discuss.how", how)
}

/// The person's Discuss on `q`, once the lead's turn that hears it is queued: kept in its answer
/// with that turn, beside any earlier one.
pub(crate) fn discuss(tx: &Transaction, q: i64, text: &str, trace: &str) -> Result<(), Error> {
    let t = read(tx, q)?;
    let mut answer = t.answer.clone().unwrap_or_else(|| json!({}));
    let entry = json!({ "text": text, "turn": t.heard_in });
    match answer["discussed"].as_array_mut() {
        Some(all) => all.push(entry),
        None => answer["discussed"] = json!([entry]),
    }
    let sql = "UPDATE tasks SET answer = ?2 WHERE number = ?1";
    tx.execute(sql, params![q, answer.to_string()])?;
    discussed(&t, "asked", trace)
        .attr("gathr.turn.id", t.heard_in.clone())
        .insert(tx)?;
    Ok(())
}

/// Each held question `sql` selects is back on the person's list as it was.
fn back(tx: &Transaction, sql: &str, args: impl Params, trace: &str) -> Result<(), Error> {
    let mut stmt = tx.prepare_cached(sql)?;
    let held: Vec<i64> = stmt
        .query_map(args, |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for q in held {
        set(tx, q, State::Ready, None, trace)?;
        discussed(&read(tx, q)?, "back", trace).insert(tx)?;
    }
    Ok(())
}

/// `turn` has ended: each question discussed in it that nothing took the place of is back on the
/// person's list as it was.
pub(crate) fn back_on_list(tx: &Transaction, turn: &str, trace: &str) -> Result<(), Error> {
    let sql = "SELECT number FROM tasks WHERE kind = 'person' AND state = 'held' AND heard_in = ?1";
    back(tx, sql, [turn], trace)
}

/// At a start: a turn still running then is cut off without ending here, so each question whose
/// lead's turn is no longer to come, or that has none, is back.
pub(crate) fn recover(tx: &Transaction, trace: &str) -> Result<(), Error> {
    let sql = "SELECT number FROM tasks t WHERE kind = 'person' AND state = 'held' AND NOT EXISTS
               (SELECT 1 FROM turns u WHERE u.id = t.heard_in AND u.status IN ('queued', 'waiting_sign_in'))";
    back(tx, sql, (), trace)
}

/// A new question from the lead's turn that heard a Discuss, for the same request, takes the
/// discussed one's place: that one is cancelled, telling nobody, and what was folded onto it moves.
pub(crate) fn replace(tx: &Transaction, new: i64, trace: &str) -> Result<(), Error> {
    let sql = "SELECT o.number FROM tasks n JOIN tasks o ON o.kind = 'person' AND o.state = 'held'
                 AND o.giver = n.giver AND o.heard_in = n.given_in AND o.request IS n.request
               WHERE n.number = ?1 ORDER BY o.number LIMIT 1";
    let Some(old) = tx
        .query_row(sql, [new], |r| r.get::<_, i64>(0))
        .optional()?
    else {
        return Ok(());
    };
    set(
        tx,
        old,
        State::Cancelled,
        Some(&format!("#{new} took its place")),
        trace,
    )?;
    // `set` cleared it as the held question settled: the request's hold reads it.
    tx.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [old])?;
    // A reason in `held::settle`'s default form names the question, so it follows; a note stays.
    let sql = "UPDATE tasks SET blocked_on = ?2,
                 reason = CASE WHEN reason = 'blocked on #' || ?1 THEN 'blocked on #' || ?2 ELSE reason END
               WHERE blocked_on = ?1 AND state = 'blocked'";
    tx.execute(sql, [old, new])?;
    discussed(&read(tx, old)?, "replaced", trace)
        .attr("gathr.task.question", new)
        .insert(tx)?;
    Ok(())
}
