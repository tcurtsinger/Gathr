//! The waiting answer in words, for an agent's `waiting` tool and `gathr diag waiting --text`.

use super::{Answer, Line, Moves};

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
        for l in of {
            let mut line = format!("- {}", l.what);
            if let Some(since) = l.since_us {
                line.push_str(&format!(", for {}", ago(now_us - since)));
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
            said.push(line);
        }
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
