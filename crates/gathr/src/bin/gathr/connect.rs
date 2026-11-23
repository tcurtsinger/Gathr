//! `gathr connect`: an MCP server over standard input and output, the way an AI app on this PC runs a local command.
//! It holds no tools of its own: each message goes to the running service's `/api/mcp` with the service's token, and
//! each answer comes back. It tells the service the app's name: `--as`, else what the app gave as it connected (its
//! `clientInfo`'s title, else its name). It never starts the service and never prints its token.

use std::io::{BufRead, Write};

use gathr::data::{DataDir, ServiceInfo};
use gathr::{Error, http};
use serde_json::{Value, json};

const NOT_RUNNING: &str = "Gathr isn't running. Open Gathr, then try again.";

pub fn run(data: &DataDir, named: Option<String>) -> Result<(), Error> {
    let mut app = named.map(|n| one_line(&n));
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if app.is_none() && message["method"] == "initialize" {
            let info = &message["params"]["clientInfo"];
            let given = info["title"].as_str().or(info["name"].as_str());
            app = given.map(one_line).filter(|n| !n.is_empty());
        }
        for answer in pass(data, &line, app.as_deref()) {
            writeln!(out, "{answer}")?;
        }
        // A request gets an answer even when the service couldn't give one; a notification gets none.
        out.flush()?;
    }
    Ok(())
}

/// The service's answers to one message, or what went wrong said as an answer to it.
fn pass(data: &DataDir, line: &str, app: Option<&str>) -> Vec<String> {
    let id = serde_json::from_str::<Value>(line)
        .ok()
        .filter(|m| m.get("method").is_some())
        .and_then(|m| m.get("id").cloned());
    let failed = |why: &str| match &id {
        Some(id) => vec![
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": why } })
                .to_string(),
        ],
        None => Vec::new(),
    };
    let Ok(info) = ServiceInfo::read(data) else {
        return failed(NOT_RUNNING);
    };
    let auth = format!("Bearer {}", info.token);
    let mut headers = vec![
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    if let Some(app) = app {
        headers.push(("x-gathr-app", app));
    }
    let answer = match http::post(info.port, "/api/mcp", &headers, line) {
        Ok(answer) => answer,
        Err(_) => return failed(NOT_RUNNING),
    };
    match answer.status {
        202 => Vec::new(),
        200 if answer
            .headers
            .to_ascii_lowercase()
            .contains("text/event-stream") =>
        {
            (answer.body.lines())
                .filter_map(|l| l.strip_prefix("data:"))
                .map(|d| d.trim().to_string())
                .filter(|d| d.starts_with('{'))
                .collect()
        }
        _ if answer.json().get("jsonrpc").is_some() => vec![answer.body.trim().to_string()],
        status => failed(&format!("Gathr answered {status}: {}", answer.body.trim())),
    }
}

/// A name on one line, as a header carries it.
fn one_line(text: &str) -> String {
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c.is_control())
        .collect();
    words
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
