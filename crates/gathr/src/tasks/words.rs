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

/// An agent's or the person's words as a sentence: a full stop is added when they end with no mark.
pub fn sentence(text: &str) -> String {
    let text = text.trim_end();
    match text.ends_with(['.', '!', '?']) {
        true => text.to_string(),
        false => format!("{text}."),
    }
}

/// A look-into task's line in its brief (0063).
const LOOK_INTO: &str = "This is a look-into task: don't change the project's files or push. Write what you make in \
                         your own folder or Shared, and name it or say what you found when you finish.";

/// The ask of a task's turn: what, why, a carry-on when it ran before, and how to finish it.
pub fn brief(
    conn: &Connection,
    t: &Task,
    giver: &str,
    carry: Option<&str>,
) -> Result<String, Error> {
    let mut lines = vec![format!("Task #{} from {giver}: {}", t.number, t.what)];
    if let Some(why) = &t.why {
        lines.push(format!("Why: {why}"));
    }
    lines.extend(super::brief::lines(t));
    if t.look_into {
        lines.push(LOOK_INTO.to_string());
    }
    if t.kind == Kind::PersonRequest {
        lines.extend(super::person::request_lines(t, giver));
    }
    if let Some(carry) = carry {
        lines.push(carry_line(carry));
    }
    if t.kind == Kind::Review {
        lines.push(
            "For a review, include the commit you reviewed when you finish the task.".to_string(),
        );
        lines.extend(super::notes::brief_lines(conn, t)?);
    }
    lines.push(match t.kind {
        Kind::PersonRequest => format!(
            "If you ask the person, their answer reaches {giver} by itself: after ask_person, end your turn with no \
             finish_task."
        ),
        _ => "Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it."
            .to_string(),
    });
    Ok(lines.join("\n"))
}

/// D's carry line: where a task's brief goes on from.
pub(crate) fn carry_line(carry: &str) -> String {
    format!(
        "Continue from here: {} Check what is already done before repeating work.",
        sentence(carry)
    )
}

/// What a task's new conversation opens with, before its ask (0055, N6): the agent's last tasks in
/// the chat but this one, and the brief when the ask isn't it (the task lost its conversation).
/// `heard`: the chat's recent lines follow it (0081). The opening, and how many tasks it lists.
pub fn opening(
    conn: &Connection,
    task: i64,
    agent: &str,
    ask: &str,
    heard: bool,
) -> Result<(String, usize), Error> {
    let t = super::records::read(conn, task)?;
    let mut lines = vec![match heard {
        true => format!(
            "This is a new conversation for task #{task}. Your earlier turns in this chat aren't in it, only \
             its recent lines below: read_chat and list_tasks have the rest."
        ),
        false => format!(
            "This is a new conversation for task #{task}. Your earlier turns in this chat aren't in it: \
             read_chat and list_tasks have them."
        ),
    }];
    let last: Vec<i64> = {
        let mut stmt = conn.prepare_cached(
            "SELECT number FROM tasks WHERE chat_id = ?1 AND owner = ?2 AND number != ?3
             ORDER BY updated_ts DESC, number DESC LIMIT 5",
        )?;
        let rows = stmt.query_map(rusqlite::params![t.chat, agent, task], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    if !last.is_empty() {
        lines.push("Your last tasks in this chat:".to_string());
    }
    for number in &last {
        lines.push(line(conn, &super::records::read(conn, *number)?)?);
    }
    let brief = brief(conn, &t, &owner_name(conn, t.giver.as_deref())?, None)?;
    if ask.lines().next() != brief.lines().next() {
        lines.push(brief);
    }
    Ok((lines.join("\n"), last.len()))
}

/// The one ask for a report, after a turn on the task ended with none (0047).
pub fn ask(t: &Task) -> String {
    format!(
        "Your last turn on task #{} ({}) ended with no report. Use finish_task to say done, left or blocked.",
        t.number, t.what
    )
}

/// The first line of what Gathr says in the chat when a task ends done (0051); its second is the
/// task's `line`. The window reads both (`ui/src/chat/hearback.ts`).
pub const DONE: &str = "A task is done:";

/// A task as a hear-back names it: "- #<number> <owner>, <what>: <how>".
pub fn line(conn: &Connection, t: &Task) -> Result<String, Error> {
    let left = t.report.as_ref().is_some_and(|r| r.how == Finish::Left);
    let how = match (t.state, &t.reason) {
        (State::Done, None) => "done".to_string(),
        // Stopped as left, or with no report: the reason says which, in the owner's words.
        (State::Blocked, Some(reason)) if left || (t.report.is_none() && t.asked) => reason.clone(),
        (state, Some(reason)) => format!("{}: {reason}", state.as_str()),
        (state, None) => state.as_str().to_string(),
    };
    let owner = owner_name(conn, t.owner.as_deref())?;
    Ok(format!("- #{} {owner}, {}: {how}", t.number, t.what))
}

/// What became of each task the giver hasn't heard of, on the giver's `own` task when it has one
/// open.
pub fn hear_back(conn: &Connection, settled: &[Task], own: Option<i64>) -> Result<String, Error> {
    // A fix held only on its merge comes back to the reviewer who gave it (hear.rs): not settled.
    let all = settled.iter().all(|t| t.state.settled());
    let mut lines = vec![match (settled.len(), all) {
        (1, true) => "The task you gave is settled:".to_string(),
        (n, true) => format!("The {n} tasks you gave are settled:"),
        (1, false) => "Where the task you gave stands:".to_string(),
        (n, false) => format!("Where the {n} tasks you gave stand:"),
    }];
    for t in settled {
        lines.push(line(conn, t)?);
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

/// A task in list_tasks, including its state and review head.
pub fn listed_line(conn: &Connection, t: &Task) -> Result<String, Error> {
    let owner = owner_name(conn, t.owner.as_deref())?;
    let reason = t
        .reason
        .as_ref()
        .map_or(String::new(), |r| format!(" ({r})"));
    let state = t.state.as_str();
    // A review says which commit it is of, and where its PR is now once that has moved past it (G42).
    let short = |commit: &str| commit.get(..7).unwrap_or(commit).to_string();
    let review = match (&t.reviewed, t.kind) {
        (Some(reviewed), Kind::Review) => {
            let at = short(reviewed);
            match (t.outdated, t.pr, &t.pr_head) {
                (true, Some(pr), Some(head)) => {
                    format!(
                        " [reviewed at {at}, out of date: #{pr} is at {}]",
                        short(head)
                    )
                }
                (true, ..) => format!(" [reviewed at {at}, out of date]"),
                _ => format!(" [reviewed at {at}]"),
            }
        }
        _ => String::new(),
    };
    Ok(format!(
        "#{} {owner} {state}{reason}{review}: {}",
        t.number, t.what
    ))
}
