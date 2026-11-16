//! The trial's file (R8): only its closed list of fields, checked against the shipped shape test.

use serde_json::{Map, Value, json};

use super::cases::{Block, Seat};

/// A run's row: who ran which case in which arm and block, how it went, and its counts.
pub fn run_row(block: &Block, seat: &Seat, outcome: &str, counts: Map<String, Value>) -> Value {
    let mut row = json!({ "case": seat.case.name(), "engine": seat.engine.name(), "model": seat.model,
        "effort": seat.effort, "arm": block.arm.name(), "block": block.n, "outcome": outcome });
    row.as_object_mut().expect("an object").extend(counts);
    row
}

/// The input tokens the runs spent, cache reads and writes included.
pub fn spent(runs: &[Value]) -> u64 {
    let of = |r: &Value, k: &str| r[k].as_u64().unwrap_or(0);
    runs.iter()
        .map(|r| of(r, "input_tokens") + of(r, "cache_read") + of(r, "cache_write"))
        .sum()
}

/// Each words kind's version, by name, and the role lines'.
pub fn words() -> Value {
    let mut words = Map::new();
    for opened in gathr::words::Opened::ALL {
        if let Some(version) = opened.version() {
            words.insert(opened.as_str().into(), version.into());
        }
    }
    words.insert("role".into(), gathr::words::ROLE.into());
    Value::Object(words)
}

/// The whole file: `head` holds its date, build and engine copies.
pub fn file(head: &Value, runs: &[Value], lines: Vec<Value>, ended: &str) -> Value {
    json!({ "format": 1, "date": head["date"], "gathr": head["gathr"], "copies": head["copies"],
        "words": words(), "lines": lines, "runs": runs, "spent": spent(runs), "ended": ended })
}
