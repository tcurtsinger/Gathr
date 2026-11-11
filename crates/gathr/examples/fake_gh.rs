//! Stands in for GitHub's `gh` in tests (`GATHR_GH_PATH`): no network, no login. It answers from
//! the JSON file `GATHR_FAKE_GH` names, `[{"args": [...], "out": "...", "code": 0}]`: the first
//! answer whose `args` begin the call's. An answer with `outs` instead gives its next one on each
//! call that matches it, then its last again, for a PR that changes between two looks. One with
//! `merged_if` (`{"repo", "base", "head"}`) gives `out_merged` once that head is in that base, as
//! GitHub marks a pushed PR merged. Each call
//! is appended to `<file>.calls`, one JSON array of its arguments a line, so a test sees what
//! Gathr asked.

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
    for (i, answer) in answers.iter().enumerate() {
        let start: Vec<String> = serde_json::from_value(answer["args"].clone()).unwrap_or_default();
        if args.len() < start.len() || args[..start.len()] != start[..] {
            continue;
        }
        let out = match answer["outs"].as_array() {
            Some(outs) => {
                let seen = bump(&file, i);
                outs.get(seen.min(outs.len().saturating_sub(1)))
                    .cloned()
                    .unwrap_or_default()
            }
            None if merged(&answer["merged_if"]) => answer["out_merged"].clone(),
            None => answer["out"].clone(),
        };
        print!("{}", out.as_str().unwrap_or_default());
        std::process::exit(answer["code"].as_i64().unwrap_or(0) as i32);
    }
    eprintln!("fake gh: no answer for {args:?}");
    std::process::exit(1);
}

/// Whether `{"repo", "base", "head"}` holds: the head reachable from the base in that repository,
/// which is when GitHub marks a pushed PR merged.
fn merged(when: &serde_json::Value) -> bool {
    let text = |k: &str| when[k].as_str().unwrap_or_default().to_string();
    let (repo, base, head) = (text("repo"), text("base"), text("head"));
    if repo.is_empty() {
        return false;
    }
    let is = std::process::Command::new("git")
        .args([
            "-C",
            &repo,
            "merge-base",
            "--is-ancestor",
            &head,
            &format!("refs/heads/{base}"),
        ])
        .status();
    is.is_ok_and(|s| s.success())
}

/// How many times answer `i` was given before, counted in `<file>.seen`.
fn bump(file: &str, i: usize) -> usize {
    let path = format!("{file}.seen");
    let mut seen: Vec<usize> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if seen.len() <= i {
        seen.resize(i + 1, 0);
    }
    let before = seen[i];
    seen[i] += 1;
    let _ = std::fs::write(&path, serde_json::to_string(&seen).unwrap_or_default());
    before
}
