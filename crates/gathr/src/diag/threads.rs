//! `gathr diag threads`: where each turn, task and line said in no turn of a chat belongs (0075), and
//! the recorded fact that tied it, or why nothing did.

use std::collections::{BTreeMap, HashSet};

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::Error;
use crate::topics::{All, TOPICS_SCHEMA, Topic, Topics};

pub fn threads(conn: &Connection, chat: &str, schema: i64) -> Result<Value, Error> {
    if !crate::store::chat_exists(conn, chat)? {
        return Err(Error::Other(format!("there's no chat {chat}")));
    }
    let topics = Topics::at_schema(conn, chat, schema)?;
    let All {
        turns,
        tasks,
        lines,
    } = topics.all();
    let opened = |id: &str| {
        let at = topics.src.at.get(id).map(|&i| &topics.src.turns[i]);
        at.and_then(|t| t.opened_by.clone())
    };
    let row = |kind: &str, id: Value, t: &Topic| {
        let mut row = json!({ "topic": t.key, "fact": t.fact, "why": t.why, "alone": t.alone,
            "broken": t.broken });
        row[kind] = id;
        row
    };
    let mut shown_turns = Vec::new();
    for (id, t) in &turns {
        let mut shown = row("turn", json!(id), t);
        shown["opened_by"] = json!(opened(id));
        shown_turns.push(shown);
    }
    let shown_tasks: Vec<Value> = tasks
        .iter()
        .map(|(n, t)| row("task", json!(n), t))
        .collect();
    let shown_lines: Vec<Value> = lines
        .iter()
        .map(|(id, t)| row("message", json!(id), t))
        .collect();
    let all = || {
        turns
            .iter()
            .map(|(_, t)| t)
            .chain(tasks.iter().map(|(_, t)| t))
            .chain(lines.iter().map(|(_, t)| t))
    };
    let mains: HashSet<&str> = all().map(|t| t.key.as_str()).collect();
    let said = turns.iter().filter(|(_, t)| SAID.contains(&t.fact)).count();
    let counts = json!({
        "said_to_person": said,
        "turns": by_fact(turns.iter().map(|(_, t)| t)),
        "tasks": by_fact(tasks.iter().map(|(_, t)| t)),
        "lines": by_fact(lines.iter().map(|(_, t)| t)),
        "main_messages": mains.len(),
        "tied_to_nothing": all().filter(|t| t.alone).count(),
        "broken": all().filter(|t| t.broken).count(),
    });
    let mut answer = json!({
        "chat": chat,
        "schema": schema,
        "steps": topics.src.steps,
        "counts": counts,
        "turns": shown_turns,
        "tasks": shown_tasks,
        "lines": shown_lines,
    });
    answer["in_words"] = in_words(&answer).into();
    Ok(answer)
}

/// The facts that make a turn's bubble a main message because it is said to the person (0078).
const SAID: [&str; 4] = ["person", "picked", "lead", "plan"];

fn by_fact<'a>(topics: impl Iterator<Item = &'a Topic>) -> BTreeMap<&'static str, usize> {
    let mut counted = BTreeMap::new();
    for t in topics {
        *counted.entry(t.fact).or_insert(0) += 1;
    }
    counted
}

fn in_words(a: &Value) -> String {
    let c = &a["counts"];
    let len = |k: &str| a[k].as_array().map_or(0, Vec::len);
    let mut out = vec![format!(
        "Chat {}: {} turns, {} tasks and {} lines said in no turn, under {} main messages (read in {} steps).",
        a["chat"].as_str().unwrap_or_default(),
        len("turns"),
        len("tasks"),
        len("lines"),
        c["main_messages"],
        a["steps"]
    )];
    if a["schema"].as_i64().is_some_and(|s| s < TOPICS_SCHEMA) {
        out.push(format!(
            "This database is at schema {}: it records no results, landing turns or line subjects.",
            a["schema"]
        ));
    }
    for kind in ["turns", "tasks", "lines"] {
        let facts = c[kind].as_object().into_iter().flatten();
        let facts: Vec<String> = facts.map(|(fact, n)| format!("{fact} {n}")).collect();
        out.push(format!("{kind} by what tied them: {}", facts.join(", ")));
    }
    out.push(format!(
        "Said to the person, each its own main message: {} turns (person: the person's message opened \
         it; picked: their message to several agents; lead: the lead on no task; plan: it wrote a plan).",
        c["said_to_person"]
    ));
    if let Some(n) = c["turns"]["placed"].as_u64() {
        out.push(format!(
            "Placed by their agent with post: {n} turns (placed: the room, or the thread it chose)."
        ));
    }
    out.push(format!(
        "Tied to nothing: {}, of which a link leads nowhere: {}.",
        c["tied_to_nothing"], c["broken"]
    ));
    for (kind, id) in [("turns", "turn"), ("tasks", "task"), ("lines", "message")] {
        out.push(String::new());
        for r in a[kind].as_array().into_iter().flatten() {
            let what = match (id, r["opened_by"].as_str()) {
                ("turn", Some(by)) => format!("turn {} ({by})", r[id].as_str().unwrap_or_default()),
                ("task", _) => format!("task #{}", r[id]),
                _ => format!("{id} {}", r[id].as_str().unwrap_or_default()),
            };
            let topic = r["topic"].as_str().unwrap_or_default();
            out.push(format!(
                "{what} -> {topic}: {}",
                r["why"].as_str().unwrap_or_default()
            ));
        }
    }
    out.join("\n")
}
