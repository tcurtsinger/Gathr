//! Stands in for GitHub's `gh` in tests (`GATHR_GH_PATH`): no network, no login. It answers from
//! the JSON file `GATHR_FAKE_GH` names, `[{"args": [...], "out": "...", "code": 0}]`: the first
//! answer whose `args` begin the call's. Each call is appended to `<file>.calls`, one JSON array of
//! its arguments a line, so a test sees what Gathr asked.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = std::env::var("GATHR_FAKE_GH").unwrap_or_default();
    let calls = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{file}.calls"));
    if let Ok(mut calls) = calls {
        let _ = writeln!(
            calls,
            "{}",
            serde_json::to_string(&args).unwrap_or_default()
        );
    }
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let answers: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
    for answer in answers {
        let start: Vec<String> = serde_json::from_value(answer["args"].clone()).unwrap_or_default();
        if args.len() >= start.len() && args[..start.len()] == start[..] {
            print!("{}", answer["out"].as_str().unwrap_or_default());
            std::process::exit(answer["code"].as_i64().unwrap_or(0) as i32);
        }
    }
    eprintln!("fake gh: no answer for {args:?}");
    std::process::exit(1);
}
