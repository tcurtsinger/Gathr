//! The waiting answer in words, for an agent's `waiting` tool and `gathr diag waiting --text`, and
//! what Gathr says of a chat where nothing moves (0052).

use super::{Answer, Line, Moves, Standing};

/// Names in a sentence: "Ann", "Ann and Bo", "Ann, Bo and Cy".
pub(super) fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => one.to_string(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

/// How long ago, to the minute.
fn ago(us: i64) -> String {
    let minutes = us.max(0) / 60_000_000;
    match minutes {
        0 => "under a minute".to_string(),
        1..60 => format!("{minutes} min"),
        60..1440 => format!("{} h {} min", minutes / 60, minutes % 60),
        _ => format!("{} d {} h", minutes / 1440, minutes % 1440 / 60),
    }
}

/// One line of the answer: what it is, for how long, and what its last look saw.
fn line(l: &Line, now_us: i64) -> String {
    let mut line = format!("- {}", l.what);
    if let Some(since) = l.since_us {
        line.push_str(&format!(", for {}", ago(now_us - since)));
    }
    if let Some(silent) = l.silent_us {
        line.push_str(&format!(", silent for {}", ago(now_us - silent)));
    }
    match (&l.seen, l.looked_us) {
        (Some(seen), Some(looked)) => {
            line.push_str(&format!(
                ". Last look, {} ago: {seen}",
                ago(now_us - looked)
            ));
        }
        (Some(seen), None) => line.push_str(&format!(". Last look: {seen}")),
        _ => {}
    }
    if let Some(error) = &l.error {
        line.push_str(&format!(". Its looks fail: {error}"));
    }
    if l.from == "service" {
        line.push_str(" (from the running service)");
    }
    line
}

/// The lines grouped by who can end each, what waits on the person first, then each agent.
pub fn text(answer: &Answer, now_us: i64) -> String {
    let mut said = Vec::new();
    // That the service isn't running comes before everything it would otherwise be moving.
    let stopped = answer.lines.iter().find(|l| l.kind == "stopped");
    said.extend(stopped.map(|l| format!("{}.", l.what)));
    let groups = [
        (Moves::You, "Waiting on you:"),
        (Moves::Nobody, "Not moving:"),
        (Moves::Itself, "Moving by itself:"),
        (Moves::Others, "Following the lines above:"),
    ];
    for (moves, heading) in groups {
        let grouped = answer.lines.iter().filter(|l| l.kind != "stopped");
        let of: Vec<&Line> = grouped.filter(|l| l.moves == moves).collect();
        if of.is_empty() {
            continue;
        }
        said.push(heading.to_string());
        said.extend(of.into_iter().map(|l| line(l, now_us)));
    }
    if said.is_empty() {
        said.push("Nothing is waiting, and no turn runs.".to_string());
    }
    let agents: Vec<String> = answer
        .agents
        .iter()
        .map(|a| {
            let task = a.task.map_or(String::new(), |n| format!(" on #{n}"));
            match a.state {
                "needs_you" => format!("{} needs you ({})", a.name, a.words),
                "working" => format!("{} is working{task}", a.name),
                "free" => format!("{} is free", a.name),
                _ => format!("{} is waiting ({})", a.name, a.words),
            }
        })
        .collect();
    if !agents.is_empty() {
        said.push(format!("Agents: {}", agents.join("; ")));
    }
    said.join("\n")
}

/// A chat where nothing moves, in a phrase: the first thing of `not_moving`, for how long, and how
/// many more there are. The answer has the longest wait first, and the rule that notices a stuck
/// chat puts what is new before it. What only follows those isn't counted.
pub fn headline(not_moving: &[&Line], agents: &[Standing], now_us: i64) -> String {
    let Some(first) = not_moving.first() else {
        return "Nothing is moving".to_string();
    };
    let whose = first.agent.as_deref();
    let who = agents.iter().find(|a| Some(a.agent.as_str()) == whose);
    let who = who.map_or("The chat", |a| a.name.as_str());
    let name = match (first.task, &first.on) {
        (Some(task), _) => format!("#{task} ({who})"),
        (None, Some(on)) => on.clone(),
        (None, None) => format!("{who}'s turn"),
    };
    // A wake goes at the first check that finds the chat stuck: "for under a minute" says nothing.
    let age = match first.silent_us.or(first.since_us) {
        Some(since) if now_us - since >= 60_000_000 => format!(" for {}", ago(now_us - since)),
        _ => String::new(),
    };
    let more = match not_moving.len() - 1 {
        0 => String::new(),
        more => format!(" and {more} more"),
    };
    format!("Not moving{age}: {name}{more}")
}

fn listed(lines: &[&Line], now_us: i64) -> String {
    let each: Vec<String> = lines.iter().map(|l| line(l, now_us)).collect();
    each.join("\n")
}

/// What Gathr asks the lead of a chat where nothing moves: `lines` is all the chat waits on.
pub(crate) fn wake(
    not_moving: &[&Line],
    lines: &[&Line],
    agents: &[Standing],
    now_us: i64,
) -> String {
    format!(
        "{}. Nothing waits on the person.\n{}\n\
         Move it on: give, restart or cancel work, or ask the person. If nothing can be done, say why.",
        headline(not_moving, agents, now_us),
        listed(lines, now_us)
    )
}

/// What Gathr tells the person of it. The window shows a line of Gathr's cut to its first line,
/// so that one carries the news.
pub(crate) fn stuck(not_moving: &[&Line], agents: &[Standing], now_us: i64) -> String {
    format!(
        "{}. Write here to move it on.\n{}",
        headline(not_moving, agents, now_us),
        listed(not_moving, now_us)
    )
}

/// What Gathr tells the lead of an agent's turn that has made no progress for `quiet_ms`.
pub(crate) fn silent(name: &str, task: Option<i64>, quiet_ms: i64) -> String {
    let on = task.map_or(String::new(), |n| format!(" on #{n}"));
    format!(
        "{name}'s turn{on} has made no progress for {}. It is still running. \
         A message to them reaches that turn.",
        ago(quiet_ms * 1000)
    )
}
