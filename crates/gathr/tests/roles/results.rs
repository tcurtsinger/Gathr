//! The trial's file, `crates/gathr/trial/results.json` (0066): the gate that holds each shipped
//! line to a kept result, and the closed list of what the file may hold (R8). The trial
//! (`tests/trial.rs`) writes the file with the same hash and checks its own output with the same list.
// Each test crate that includes it uses part of it.
#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Where the trial's file is kept.
pub fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("trial/results.json")
}

/// What the gate compares: the lowercase hex SHA-256 of a line's exact text.
pub fn line_sha256(text: &str) -> String {
    gathr::fetch::sha256(text.as_bytes())
}

/// Each shipped line, `(id, text)`, that has no row in `file` with its text's hash and `kept`.
pub fn unkept(file: &Value, shipped: &[(&str, &str)]) -> Vec<String> {
    let rows = file["lines"].as_array().cloned().unwrap_or_default();
    let kept = |id: &str, sha: &str| {
        rows.iter()
            .any(|r| r["id"] == id && r["sha256"] == sha && r["decision"] == "kept")
    };
    shipped
        .iter()
        .filter(|(id, text)| !kept(id, &line_sha256(text)))
        .map(|(id, text)| format!("{id} has no kept result for its text: {text:?}"))
        .collect()
}

const TOP: &[&str] = &[
    "format", "date", "gathr", "copies", "words", "lines", "runs", "spent", "ended",
];
const LINE: &[&str] = &["id", "sha256", "case", "decision"];
const CASES: &[&str] = &["T1", "T2", "T5", "T8"];
const OUTCOMES: &[&str] = &[
    "passed",
    "failed",
    "compared",
    "no_question",
    "spoiled",
    "over_cap",
    "timed_out",
    "usage_limit",
    "signed_out",
];
const NAMED: &[&str] = &["none", "camel", "snake", "kebab", "key style", "key case"];
/// A run's counts: numbers or booleans.
const COUNTS: &[&str] = &[
    "block",
    "src_lines",
    "test_lines",
    "new_src_files",
    "extra_exports",
    "extra_options",
    "extra_params",
    "guard_lines",
    "over_built",
    "hidden_test",
    "reported_done",
    "questions",
    "with_choices",
    "reaction",
    "message_calls",
    "reply_chars",
    "wrong_on_fault",
    "unasked_on_planted",
    "wrong_elsewhere",
    "input_tokens",
    "cache_read",
    "cache_write",
    "output_tokens",
    "turns",
    "tool_calls",
    "approvals",
    "seconds",
];

/// Each way `file` holds something outside R8's closed list: a field not on it, or a value not of
/// its kind. No path, name, id, email or free text fits any kind.
pub fn shape(file: &Value) -> Vec<String> {
    let mut bad = Vec::new();
    let Some(top) = file.as_object() else {
        return vec!["the file isn't an object".into()];
    };
    fields(&mut bad, "the file", top, TOP);
    if file["format"] != 1 {
        bad.push("format isn't 1".into());
    }
    if !file["lines"].is_array() {
        bad.push("lines isn't a list".into());
    }
    for (k, v) in top {
        let ok = match k.as_str() {
            "date" => is(v, is_date),
            "gathr" => is(v, |s| (7..=40).contains(&s.len()) && hex(s)),
            "copies" => each(v, |k, v| {
                ["claude", "codex"].contains(&k) && is(v, is_version)
            }),
            // `plan_check` went with the one-checker plan check (0080): the trial of 2026-11-16 has it.
            "words" => each(v, |k, v| {
                (["role", "plan_check"].contains(&k)
                    || gathr::words::Opened::ALL.iter().any(|o| o.as_str() == k))
                    && v.is_u64()
            }),
            "spent" => v.is_u64(),
            "ended" => one_of(v, &["done", "cap", "usage_limit", "signed_out"]),
            _ => true,
        };
        if !ok {
            bad.push(format!("{k} isn't of its kind: {v}"));
        }
    }
    for (i, line) in rows(&mut bad, file, "lines") {
        fields(&mut bad, &format!("lines/{i}"), line, LINE);
        for (k, v) in line {
            let ok = match k.as_str() {
                "id" => one_of(v, &["L1", "L2", "L3", "L4"]),
                "sha256" => is(v, |s| s.len() == 64 && hex(s)),
                "case" => one_of(v, CASES),
                "decision" => one_of(v, &["kept", "dropped", "undecided"]),
                _ => true,
            };
            if !ok {
                bad.push(format!("lines/{i}/{k} isn't of its kind: {v}"));
            }
        }
    }
    for (i, run) in rows(&mut bad, file, "runs") {
        for (k, v) in run {
            let ok = match k.as_str() {
                "case" => one_of(v, CASES),
                "engine" => one_of(v, &["claude", "codex"]),
                "model" | "effort" => is(v, is_model),
                "arm" => one_of(v, &["with", "without"]),
                "outcome" => one_of(v, OUTCOMES),
                "named_answered" => one_of(v, NAMED),
                k if COUNTS.contains(&k) => v.is_u64() || v.is_boolean(),
                _ => {
                    bad.push(format!("runs/{i} has a field not on the list: {k}"));
                    continue;
                }
            };
            if !ok {
                bad.push(format!("runs/{i}/{k} isn't of its kind: {v}"));
            }
        }
    }
    bad
}

fn fields(bad: &mut Vec<String>, at: &str, object: &Map<String, Value>, allowed: &[&str]) {
    for key in object.keys().filter(|k| !allowed.contains(&k.as_str())) {
        bad.push(format!("{at} has a field not on the list: {key}"));
    }
}

/// The objects of the list `key`, each with its place; anything else in it is a finding.
fn rows<'a>(
    bad: &mut Vec<String>,
    file: &'a Value,
    key: &str,
) -> Vec<(usize, &'a Map<String, Value>)> {
    let list = file[key].as_array().map(Vec::as_slice).unwrap_or_default();
    let mut objects = Vec::new();
    for (i, row) in list.iter().enumerate() {
        match row.as_object() {
            Some(object) => objects.push((i, object)),
            None => bad.push(format!("{key}/{i} isn't an object: {row}")),
        }
    }
    objects
}

fn each(object: &Value, ok: impl Fn(&str, &Value) -> bool) -> bool {
    object
        .as_object()
        .is_some_and(|map| map.iter().all(|(k, v)| ok(k, v)))
}

fn is(value: &Value, ok: impl Fn(&str) -> bool) -> bool {
    value.as_str().is_some_and(ok)
}

fn one_of(value: &Value, list: &[&str]) -> bool {
    is(value, |s| list.contains(&s))
}

fn hex(s: &str) -> bool {
    s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

/// `x.y.z`, digits first.
fn is_version(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

/// A model or effort as an engine names it: `opus`, `gpt-6.1-sol`, `latest:sol`.
fn is_model(s: &str) -> bool {
    let named = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | ':' | '_')
    };
    !s.is_empty() && s.len() <= 64 && s.chars().all(named)
}
