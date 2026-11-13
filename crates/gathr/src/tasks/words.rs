//! What Gathr tells an agent about its tasks: the brief a task's turn starts with, and the
//! hear-back once the tasks it gave are settled. Short: it's in every one of those turns (0020).

use rusqlite::{Connection, OptionalExtension};

use super::{Finish, Kind, State, Task};
use crate::Error;

pub fn name_of(conn: &Connection, agent: &str) -> Result<String, Error> {
    let name: Option<String> = conn
        .query_row("SELECT name FROM agents WHERE id = ?1", [agent], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(name.unwrap_or_else(|| "a deleted agent".to_string()))
}

/// A task's owner by name: none is the person.
pub fn owner_name(conn: &Connection, owner: Option<&str>) -> Result<String, Error> {
    match owner {
        Some(agent) => name_of(conn, agent),
        None => Ok("the person".to_string()),
    }
}

/// The ask of a task's turn: what, why, a carry-on when it ran before, and how to finish it.
pub fn brief(t: &Task, giver: &str, carry: Option<&str>) -> String {
    let mut lines = vec![format!("Task #{} from {giver}: {}", t.number, t.what)];
    if let Some(why) = &t.why {
        lines.push(format!("Why: {why}"));
    }
    if t.kind == Kind::PersonRequest {
        lines.extend(super::person::request_lines(t, giver));
    }
    if let Some(carry) = carry {
        lines.push(format!(
            "Continue from here: {carry}. Check what is already done before repeating work."
        ));
    }
    if t.kind == Kind::Review {
        lines.push(
            "For a review, include the commit you reviewed when you finish the task.".to_string(),
        );
    }
    lines.push(
        "Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it."
            .to_string(),
    );
    lines.join("\n")
}

/// The one ask for a report, after a turn on the task ended with none (0047).
pub fn ask(t: &Task) -> String {
    format!(
        "Your last turn on task #{} ({}) ended with no report. Use finish_task to say done, left or blocked.",
        t.number, t.what
    )
}

/// What became of each task the giver gave in one turn, on the giver's `own` task when it has one
/// open.
pub fn hear_back(conn: &Connection, settled: &[Task], own: Option<i64>) -> Result<String, Error> {
    // A task held only on its merge comes back too (held.rs): not settled yet.
    let all = settled.iter().all(|t| t.state.settled());
    let mut lines = vec![match (settled.len(), all) {
        (1, true) => "The task you gave is settled:".to_string(),
        (n, true) => format!("The {n} tasks you gave are settled:"),
        (1, false) => "Where the task you gave stands:".to_string(),
        (n, false) => format!("Where the {n} tasks you gave stand:"),
    }];
    for t in settled {
        let left = t.report.as_ref().is_some_and(|r| r.how == Finish::Left);
        let how = match (t.state, &t.reason) {
            (State::Done, None) => "done".to_string(),
            // Stopped as left, or with no report: the reason says which, in the owner's words.
            (State::Blocked, Some(reason)) if left || (t.report.is_none() && t.asked) => {
                reason.clone()
            }
            (state, Some(reason)) => format!("{}: {reason}", state.as_str()),
            (state, None) => state.as_str().to_string(),
        };
        let owner = owner_name(conn, t.owner.as_deref())?;
        lines.push(format!("- #{} {owner}, {}: {how}", t.number, t.what));
    }
    // One closing line: the window reads a hear-back's lines (`ui/src/chat/hearback.ts`).
    let mut close = "Read replies with read_chat, then continue.".to_string();
    if let Some(own) = own {
        close.push_str(&format!(
            " This turn is on your task #{own}: end it with finish_task."
        ));
    }
    lines.push(close);
    Ok(lines.join("\n"))
}

/// A wait in a few words: on whom, or for what.
pub fn wait_name(
    conn: &Connection,
    kind: &str,
    target: Option<&str>,
    parts: i64,
) -> Result<String, Error> {
    let target = target.unwrap_or_default();
    Ok(match kind {
        "agent" => name_of(conn, target)?,
        "ci" if target.chars().all(|c| c.is_ascii_digit()) => format!("CI on #{target}"),
        "ci" => format!("CI on {}", target.get(..7).unwrap_or(target)),
        "pr" => format!("#{target} to merge or close"),
        "up_to_date" => format!("#{target} to be up to date"),
        "all" => format!("all of {parts} waits"),
        _ => "a check".to_string(),
    })
}
