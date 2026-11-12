//! Stand-in for Codex 0.166.0 app-server. Recorded turn frames, synthetic auth and pagination.
use serde_json::{Value, json};
use std::io::{BufRead, Write};

fn send(value: &Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}

pub fn run() {
    let folder = std::path::PathBuf::from(std::env::var_os("CODEX_HOME").unwrap_or_default());
    let recorded = std::env::var("GATHR_REPLAY")
        .or_else(|_| super::recording_beside_me())
        .unwrap_or_default();
    let text = std::fs::read_to_string(recorded).unwrap_or_default();
    let rows: Vec<Value> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut stdin = std::io::stdin().lock().lines();
    while let Some(Ok(line)) = stdin.next() {
        let v: Value = serde_json::from_str(&line).unwrap_or_default();
        let p = &v["params"];
        let result = match v["method"].as_str().unwrap_or_default() {
            "initialize" => json!({"userAgent":"gathr/0.166.0 (test)"}),
            "initialized" => continue,
            "account/login/start" => {
                let device = p["type"] == "chatgptDeviceCode";
                let result = if device {
                    json!({"type":"chatgptDeviceCode", "loginId":"test-login", "verificationUrl":"https://codex.example/device", "userCode":"ABCD-1234"})
                } else {
                    json!({"type":"chatgpt", "loginId":"test-login", "authUrl":"https://codex.example/sign-in"})
                };
                send(&json!({"id":v["id"], "result":result}));
                // A hold file lets a live UI check switch methods or cancel before completion.
                let hold = folder.join("hold-login");
                if hold.is_file() {
                    continue;
                }
                let failed = folder.join("fail-login").is_file();
                if !failed {
                    std::fs::write(
                        folder.join("auth.json"),
                        r#"{"tokens":{"refresh_token":"made-up"}}"#,
                    )
                    .unwrap();
                }
                send(
                    &json!({"method":"account/login/completed", "params":{"loginId":"test-login", "success":!failed,
                    "error":if failed {json!("The test sign-in was refused")} else {Value::Null}}}),
                );
                continue;
            }
            "account/logout" => {
                let _ = std::fs::remove_file(folder.join("auth.json"));
                json!({})
            }
            "account/login/cancel" => json!({"status":"canceled"}),
            "model/list" => {
                let mut models: Value =
                    serde_json::from_str(include_str!("../../tests/frames/codex_models.json"))
                        .unwrap();
                let all = models["data"].as_array().unwrap();
                let page = if p["cursor"] == "page-2" {
                    all[1..].to_vec()
                } else {
                    vec![all[0].clone()]
                };
                models["data"] = page.into();
                models["nextCursor"] = if p["cursor"] == "page-2" {
                    Value::Null
                } else {
                    json!("page-2")
                };
                models
            }
            "thread/start" | "thread/resume" => {
                let recorded = rows
                    .iter()
                    .filter_map(|r| serde_json::from_str::<Value>(r["line"].as_str()?).ok())
                    .find(|r| r["result"]["thread"]["id"].is_string());
                let mut result = recorded
                    .map(|v| v["result"].clone())
                    .unwrap_or(json!({"thread":{"id":"test-thread"}, "model":"gpt-6.1-sol"}));
                let sandbox = match p["sandbox"].as_str() {
                    Some("read-only") => "readOnly",
                    Some("danger-full-access") => "dangerFullAccess",
                    _ => "workspaceWrite",
                };
                result["sandbox"] = json!({"type":sandbox});
                result["approvalPolicy"] = p["approvalPolicy"].clone();
                result
            }
            "turn/start" => {
                // Replay the turn segment, past handshake/thread frames. Input rows wait for approvals.
                let at = rows.iter().position(|r| {
                    r["dir"] == "in" && r["line"].as_str().is_some_and(|l| l.contains("turn/start"))
                });
                if let Some(at) = at {
                    for row in &rows[at + 1..] {
                        match row["dir"].as_str() {
                            Some("out") => {
                                let mut frame: Value =
                                    serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
                                if frame["result"]["turn"].is_object() {
                                    frame["id"] = v["id"].clone();
                                }
                                send(&frame);
                            }
                            Some("err") => {
                                eprintln!("{}", row["line"].as_str().unwrap_or_default())
                            }
                            Some("in") => {
                                if stdin.next().is_none() {
                                    return;
                                }
                            }
                            Some("sleep") => {
                                let ms = row["line"].as_str().and_then(|ms| ms.parse().ok());
                                std::thread::sleep(std::time::Duration::from_millis(
                                    ms.unwrap_or(0),
                                ));
                            }
                            Some("hold") => {
                                let held =
                                    std::env::var_os("GATHR_REPLAY_HOLD").unwrap_or_default();
                                while !held.is_empty() && !std::path::Path::new(&held).exists() {
                                    std::thread::sleep(std::time::Duration::from_millis(10));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                continue;
            }
            "turn/interrupt" => json!({}),
            _ => {
                send(
                    &json!({"id":v["id"], "error":{"code":-32601, "message":"test method unsupported"}}),
                );
                continue;
            }
        };
        send(&json!({"id":v["id"], "result":result}));
    }
}
