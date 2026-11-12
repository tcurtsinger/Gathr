//! `codex app-server`'s JSON-RPC, read into the same `Frame`s Claude Code's output is (0033).
//! A `Session` is one engine's side of the conversation: what it asked and is waiting for, the
//! thread and the turn under way, and messages held until the thread is open.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::claude::frames::{Frame, TurnEnd, Usage};

/// How the engine's thread opens once `initialize` has answered.
#[derive(Debug, Clone, Default)]
pub struct Opening {
    /// The chat's saved thread, resumed; a new one when `None`.
    pub resume: Option<String>,
    pub cwd: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// `(sandbox, approvalPolicy, approvalsReviewer)`, from `command::policy`.
    pub policy: (&'static str, &'static str, &'static str),
    pub instructions: Option<String>,
    /// The agent's folders beside `cwd` its sandbox writes in (0034): its own and Shared.
    pub writable: Vec<String>,
    /// Where its conversation compacts, in tokens (Auto-compact, K3); the engine's own when `None`.
    pub compact_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Asked {
    Initialize,
    Models,
    Thread,
    Turn,
    Steer,
    Interrupt,
    Other,
}

#[derive(Default)]
pub struct Session {
    next: u64,
    asked: HashMap<u64, Asked>,
    opening: Option<Opening>,
    models: Vec<Value>,
    cursors: std::collections::HashSet<String>,
    thread: Option<String>,
    turn: Option<String>,
    starting: bool,
    stop_pending: bool,
    /// Messages sent before the thread opened, in order.
    held: Vec<String>,
    /// A compaction asked before the thread opened.
    compact_held: bool,
    /// Approvals Codex is waiting on, by the id Gathr shows: their JSON-RPC id.
    approvals: HashMap<String, Value>,
    replied: bool,
    total: Option<Usage>,
    baseline: Option<Usage>,
    summed: Usage,
    /// Each window's reset time (Unix seconds) and name, from the last `account/rateLimits/updated`.
    limits: Vec<(i64, String)>,
    /// The engine's version, once `initialize` answered.
    pub version: Option<String>,
}

/// What one line meant, and what to write back.
#[derive(Debug, Default)]
pub struct Read {
    pub frames: Vec<Frame>,
    pub send: Vec<Value>,
}

impl Session {
    /// A new session and its first line: `initialize`. The thread opens when it answers.
    pub fn open(opening: Opening) -> (Session, Value) {
        let mut session = Session {
            opening: Some(opening),
            ..Session::default()
        };
        let params = json!({ "clientInfo": { "name": "gathr", "title": "Gathr",
            "version": env!("CARGO_PKG_VERSION") } });
        let first = session.request(Asked::Initialize, "initialize", params);
        (session, first)
    }

    /// The handshake alone, for asking the engine something without a thread (models, sign-in).
    pub fn handshake() -> (Session, Value) {
        let (mut session, first) = Session::open(Opening::default());
        session.opening = None;
        (session, first)
    }

    fn request(&mut self, asked: Asked, method: &str, params: Value) -> Value {
        self.next += 1;
        self.asked.insert(self.next, asked);
        json!({ "id": self.next, "method": method, "params": params })
    }

    /// Another request, its answer read as `Frame::Other`; the id it went out with.
    pub fn ask(&mut self, method: &str, params: Value) -> (u64, Value) {
        let line = self.request(Asked::Other, method, params);
        (self.next, line)
    }

    /// A message: a new turn, or added to the one under way. Held until the thread is open.
    pub fn user(&mut self, text: &str) -> Option<Value> {
        if self.starting {
            self.held.push(text.to_string());
            return None;
        }
        let Some(thread) = self.thread.clone() else {
            self.held.push(text.to_string());
            return None;
        };
        let input = json!([{ "type": "text", "text": text }]);
        Some(match self.turn.clone() {
            Some(turn) => self.request(
                Asked::Steer,
                "turn/steer",
                json!({ "threadId": thread, "expectedTurnId": turn, "input": input }),
            ),
            None => {
                self.starting = true;
                let effort = self.opening.as_ref().and_then(|o| o.effort.clone());
                // Asked for a summary, Codex starts a reasoning item when the thinking begins
                // and completes it when it ends (0042). The summary itself is never read.
                let mut params = json!({ "threadId": thread, "input": input, "summary": "auto" });
                if let Some(effort) = effort {
                    params["effort"] = effort.into();
                }
                self.request(Asked::Turn, "turn/start", params)
            }
        })
    }

    /// Compacts the thread, which Codex runs as a turn of its own. Held until the thread is open.
    pub fn compact(&mut self) -> Option<Value> {
        let Some(thread) = self.thread.clone().filter(|_| !self.starting) else {
            self.compact_held = true;
            return None;
        };
        self.starting = true;
        let params = json!({ "threadId": thread });
        Some(self.request(Asked::Turn, "thread/compact/start", params))
    }

    /// Stop: Codex's own interrupt of the turn under way, never a kill.
    pub fn interrupt(&mut self) -> Option<Value> {
        let (Some(thread), Some(turn)) = (self.thread.clone(), self.turn.clone()) else {
            self.stop_pending = true;
            return None;
        };
        self.stop_pending = false;
        let params = json!({ "threadId": thread, "turnId": turn });
        Some(self.request(Asked::Interrupt, "turn/interrupt", params))
    }

    /// The person's answer to an approval Codex asked for.
    pub fn approval(&mut self, request_id: &str, allow: bool) -> Option<Value> {
        let id = self.approvals.remove(request_id)?;
        let decision = if allow { "accept" } else { "decline" };
        Some(json!({ "id": id, "result": { "decision": decision } }))
    }

    pub fn read(&mut self, value: &Value) -> Read {
        let mut read = Read::default();
        match (value.get("method").and_then(Value::as_str), value.get("id")) {
            (Some(method), Some(id)) => {
                self.server_request(method, id, &value["params"], &mut read)
            }
            (Some(method), None) => self.notification(method, &value["params"], &mut read),
            (None, Some(id)) => self.answer(id, value, &mut read),
            (None, None) => read.frames.push(Frame::Other("unknown".into())),
        }
        read
    }

    fn answer(&mut self, id: &Value, value: &Value, read: &mut Read) {
        let asked = id.as_u64().and_then(|id| self.asked.remove(&id));
        let result = &value["result"];
        let error = value.get("error").filter(|e| !e.is_null());
        match (asked, error) {
            (Some(Asked::Initialize), None) => {
                let agent = result["userAgent"].as_str().unwrap_or("");
                match super::command::usable(agent) {
                    Ok(version) => {
                        self.version = Some(version);
                        read.send
                            .push(json!({ "method": "initialized", "params": {} }));
                        if self
                            .opening
                            .as_ref()
                            .and_then(|o| o.model.as_deref())
                            .is_some_and(|m| m.starts_with(super::models::LATEST))
                        {
                            let request = self.request(
                                Asked::Models,
                                "model/list",
                                json!({"includeHidden":false}),
                            );
                            read.send.push(request);
                        } else {
                            read.send.extend(self.open_thread());
                        }
                    }
                    Err(why) => read.frames.push(Frame::EngineError {
                        kind: "engine_version".into(),
                        message: why,
                    }),
                }
            }
            (Some(Asked::Models), None) => match self.model_page(result) {
                Ok(next) => read.send.extend(next),
                Err(message) => read.frames.push(Frame::EngineError {
                    kind: "models".into(),
                    message,
                }),
            },
            (Some(Asked::Thread), None) => {
                let thread = result["thread"]["id"].as_str().unwrap_or("").to_string();
                self.thread = Some(thread.clone());
                let sandbox = result["sandbox"]["type"].as_str().unwrap_or("");
                let policy = result["approvalPolicy"].as_str().unwrap_or("");
                read.frames.push(Frame::Init {
                    session_id: thread,
                    model: result["model"].as_str().unwrap_or("").to_string(),
                    tools: Vec::new(),
                    permission_mode: format!("{sandbox} {policy}"),
                });
                if self.stop_pending {
                    self.stop_pending = false;
                    self.held.clear();
                    self.compact_held = false;
                    read.frames.push(Frame::TurnEnd(ended("interrupted")));
                } else {
                    self.flush_held(read);
                }
            }
            (Some(Asked::Turn), None) => {
                self.turn = result["turn"]["id"].as_str().map(String::from);
                self.starting = false;
                if self.stop_pending {
                    read.send.extend(self.interrupt());
                }
                self.flush_held(read);
            }
            (
                Some(asked @ (Asked::Initialize | Asked::Models | Asked::Thread | Asked::Turn)),
                Some(error),
            ) => {
                read.frames.push(Frame::EngineError {
                    kind: format!("{asked:?}").to_lowercase(),
                    message: error["message"].as_str().unwrap_or("").to_string(),
                });
                if asked == Asked::Turn {
                    read.frames.push(Frame::TurnEnd(failed(error)));
                }
            }
            (Some(asked @ (Asked::Steer | Asked::Interrupt)), _) => {
                let ok = error.is_none();
                if asked == Asked::Steer
                    && let Some(error) = error
                {
                    read.frames.push(Frame::EngineError {
                        kind: "steer".into(),
                        message: error["message"]
                            .as_str()
                            .unwrap_or("Codex rejected the message")
                            .into(),
                    });
                }
                let request_id = format!("{asked:?}").to_lowercase();
                read.frames.push(Frame::ControlAnswer { request_id, ok });
            }
            _ => read.frames.push(Frame::Other("answer".into())),
        }
    }

    fn flush_held(&mut self, read: &mut Read) {
        if std::mem::take(&mut self.compact_held) {
            read.send.extend(self.compact());
        }
        for text in std::mem::take(&mut self.held) {
            read.send.extend(self.user(&text));
        }
    }

    fn model_page(&mut self, page: &Value) -> Result<Option<Value>, String> {
        let data = page["data"]
            .as_array()
            .ok_or("Codex returned no model list")?;
        self.models.extend(data.iter().cloned());
        if !page["nextCursor"].is_null() {
            let cursor = page["nextCursor"]
                .as_str()
                .ok_or("Codex returned an invalid model cursor")?;
            if !self.cursors.insert(cursor.to_string()) {
                return Err("Codex repeated a model-list cursor".into());
            }
            return Ok(Some(self.request(
                Asked::Models,
                "model/list",
                json!({"cursor":cursor,"includeHidden":false}),
            )));
        }
        let rows = super::models::listed(&std::mem::take(&mut self.models))?;
        let opening = self.opening.as_mut().ok_or("Codex has no thread to open")?;
        let model = rows
            .iter()
            .find(|r| r["id"].as_str() == opening.model.as_deref())
            .and_then(|r| r["resolved"].as_str())
            .ok_or("the account no longer offers this model family")?;
        opening.model = Some(model.to_string());
        Ok(self.open_thread())
    }

    fn open_thread(&mut self) -> Option<Value> {
        let o = self.opening.clone()?;
        let (sandbox, approval, reviewer) = o.policy;
        let mut params = json!({ "cwd": o.cwd, "sandbox": sandbox, "approvalPolicy": approval,
            "approvalsReviewer": reviewer, "developerInstructions": o.instructions });
        if let Some(model) = &o.model {
            params["model"] = model.clone().into();
        }
        // Edits and Auto: writes stay in the agent's folders, the network is on as for Claude
        // agents (the owner's call, 2026-11-10), so `gh` and `git fetch` work; pushes still ask.
        let mut config = serde_json::Map::new();
        if sandbox == "workspace-write" {
            let mut table = json!({ "network_access": true });
            if !o.writable.is_empty() {
                table["writable_roots"] = json!(o.writable);
            }
            config.insert("sandbox_workspace_write".into(), table);
        }
        if let Some(at) = o.compact_at {
            config.insert("model_auto_compact_token_limit".into(), at.into());
        }
        if !config.is_empty() {
            params["config"] = config.into();
        }
        Some(match &o.resume {
            Some(thread) => {
                params["threadId"] = thread.clone().into();
                params["excludeTurns"] = true.into();
                self.request(Asked::Thread, "thread/resume", params)
            }
            None => self.request(Asked::Thread, "thread/start", params),
        })
    }

    fn notification(&mut self, method: &str, params: &Value, read: &mut Read) {
        let item = &params["item"];
        let text = |v: &Value| v.as_str().unwrap_or("").to_string();
        match method {
            "turn/started" => {
                self.turn = params["turn"]["id"].as_str().map(String::from);
                self.starting = false;
                if self.stop_pending {
                    read.send.extend(self.interrupt());
                }
                self.flush_held(read);
                self.replied = false;
                self.baseline = self.total;
                self.summed = Usage::default();
            }
            "item/agentMessage/delta" => {
                self.replied(read);
                read.frames.push(Frame::TextDelta(text(&params["delta"])));
            }
            "item/started" if item["type"] == "userMessage" => {
                let parts = item["content"].as_array().into_iter().flatten();
                let joined: Vec<&str> = parts.filter_map(|p| p["text"].as_str()).collect();
                read.frames.push(Frame::Read(joined.join("")));
            }
            "item/started" if item["type"] == "reasoning" => {
                self.replied(read);
                read.frames.push(Frame::Thinking { tokens: None });
            }
            "item/started" => {
                self.replied(read);
                if let Some(name) = tool_name(item) {
                    let input = super::steps::step(item);
                    let missing = input.as_ref().err().map(|field| Frame::ToolShape {
                        tool: name.clone(),
                        field,
                    });
                    read.frames.push(Frame::ToolStart {
                        id: text(&item["id"]),
                        name,
                        input: input.unwrap_or(None),
                    });
                    read.frames.extend(missing);
                }
            }
            // Once, as it completes; Codex 0.166.0 sent no deprecated `thread/compacted` with it.
            "item/completed" if item["type"] == "contextCompaction" => {
                read.frames.push(Frame::Compacted {
                    trigger: "auto".into(),
                    before: None,
                    after: None,
                });
            }
            // Without a summary a `reasoning` item starts and completes together, once the
            // thinking is over (0042): the service tells that mark from a thought it saw run.
            "item/completed" if item["type"] == "reasoning" => read.frames.push(Frame::Thought),
            "item/completed" if item["type"] == "agentMessage" => {
                read.frames.push(Frame::Text(text(&item["text"])));
            }
            "item/completed" if tool_name(item).is_some() => {
                // A web search and an image view end with neither: they have no way to fail.
                let unsaid = item.get("status").is_none() && item.get("success").is_none();
                let ok = item["status"] == "completed" || item["success"] == true || unsaid;
                read.frames.push(Frame::ToolEnd {
                    id: text(&item["id"]),
                    ok,
                    input: super::steps::step(item).unwrap_or(None),
                    result: super::steps::result(item),
                    told: true,
                });
            }
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"];
                self.total = Some(usage_of(&usage["total"]));
                add(&mut self.summed, &usage_of(&usage["last"]));
                // A compaction's usage reads as a call with no input (Codex 0.166.0): no call ran, so
                // the last real call's fill stands until the next one.
                if let Some(used) = usage["last"]["inputTokens"].as_i64().filter(|&u| u > 0) {
                    read.frames.push(Frame::Fill { used, model: None });
                }
                if let Some(window) = usage["modelContextWindow"].as_i64() {
                    read.frames.push(Frame::Windows(vec![(None, window)]));
                }
            }
            "account/rateLimits/updated" => self.limits = windows(&params["rateLimits"]),
            "error" if params["willRetry"] != true => {
                let error = &params["error"];
                let kind = error_kind(error);
                if kind == "usageLimitExceeded" {
                    read.frames.extend(self.limit());
                }
                let message = text(&error["message"]);
                read.frames.push(Frame::EngineError { kind, message });
            }
            "turn/completed" => {
                let turn = &params["turn"];
                let mut end = match turn["status"].as_str() {
                    Some("completed") => ended("completed"),
                    Some("interrupted") => ended("interrupted"),
                    _ => failed(&turn["error"]),
                };
                end.usage = self.turn_usage();
                end.duration_ms = turn["durationMs"].as_i64();
                self.turn = None;
                self.starting = false;
                self.stop_pending = false;
                read.frames.push(Frame::TurnEnd(end));
            }
            other => read.frames.push(Frame::Other(other.to_string())),
        }
    }

    fn replied(&mut self, read: &mut Read) {
        if !std::mem::replace(&mut self.replied, true) {
            read.frames.push(Frame::Replied);
        }
    }

    /// The turn's tokens: the thread's running total since the turn began, or the sum of each
    /// call's own when no total was seen before it (0001: never add running totals).
    fn turn_usage(&self) -> Usage {
        match (self.baseline, self.total) {
            (Some(before), Some(now)) => Usage {
                input: now.input - before.input,
                output: now.output - before.output,
                cache_read: now.cache_read - before.cache_read,
                cache_write: now.cache_write - before.cache_write,
            },
            _ => self.summed,
        }
    }

    /// The account's limit, from the latest window that ran out (0010's usage hold).
    fn limit(&self) -> Option<Frame> {
        let (resets_at, kind) = self.limits.iter().max_by_key(|(at, _)| *at)?.clone();
        Some(Frame::Limit {
            status: "rejected".into(),
            resets_at: Some(resets_at),
            kind: Some(kind),
        })
    }

    fn server_request(&mut self, method: &str, id: &Value, params: &Value, read: &mut Read) {
        let request_id = id.to_string();
        let asked = |tool: &str, input: Value| Frame::ApprovalAsked {
            request_id: request_id.clone(),
            tool: tool.to_string(),
            input,
            why: params["reason"].as_str().map(String::from),
        };
        match method {
            "item/commandExecution/requestApproval" => {
                let input = json!({ "command": params["command"], "cwd": params["cwd"] });
                read.frames.push(asked("Shell", input));
            }
            "item/fileChange/requestApproval" => {
                let input = json!({ "grantRoot": params["grantRoot"] });
                read.frames.push(asked("Edit", input));
            }
            // Nothing else Codex asks of a client is Gathr's to give: refused, so it never waits.
            _ => {
                read.send.push(json!({ "id": id, "error": { "code": -32601,
                    "message": format!("Gathr doesn't answer {method}") } }));
                read.frames.push(Frame::Other(method.to_string()));
                return;
            }
        }
        self.approvals.insert(request_id, id.clone());
    }
}

/// The tool row's name for an item that's a tool call, as Gathr shows it.
fn tool_name(item: &Value) -> Option<String> {
    let s = |key: &str| item[key].as_str().unwrap_or("");
    Some(match s("type") {
        "commandExecution" => "Shell".into(),
        "fileChange" => "Edit".into(),
        "mcpToolCall" => format!("mcp__{}__{}", s("server"), s("tool")),
        "dynamicToolCall" => s("tool").into(),
        "webSearch" => "WebSearch".into(),
        "imageView" => "ViewImage".into(),
        _ => return None,
    })
}

fn usage_of(v: &Value) -> Usage {
    let n = |key: &str| v[key].as_i64().unwrap_or(0);
    Usage {
        // Codex counts cached input inside its input; Gathr keeps them apart, as Claude reports.
        input: n("inputTokens") - n("cachedInputTokens"),
        output: n("outputTokens"),
        cache_read: n("cachedInputTokens"),
        cache_write: n("cacheWriteInputTokens"),
    }
}

fn add(to: &mut Usage, more: &Usage) {
    to.input += more.input;
    to.output += more.output;
    to.cache_read += more.cache_read;
    to.cache_write += more.cache_write;
}

fn windows(limits: &Value) -> Vec<(i64, String)> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|name| {
            let window = &limits[*name];
            let at = window["resetsAt"].as_i64()?;
            (window["usedPercent"].as_i64()? >= 100).then(|| (at, name.to_string()))
        })
        .collect()
}

/// The error's kind: `authentication_failed` for a 401 (as Claude Code's), else Codex's own.
fn error_kind(error: &Value) -> String {
    let info = &error["codexErrorInfo"];
    let status = info
        .as_object()
        .and_then(|o| o.values().next())
        .and_then(|v| v["httpStatusCode"].as_i64());
    match (status, info) {
        (Some(401), _) => "authentication_failed".into(),
        (_, Value::String(kind)) => kind.clone(),
        (_, Value::Object(o)) => o.keys().next().cloned().unwrap_or_default(),
        _ => "error".into(),
    }
}

fn ended(status: &str) -> TurnEnd {
    TurnEnd {
        ok: status == "completed",
        subtype: status.into(),
        reason: Some(status.into()),
        ..TurnEnd::default()
    }
}

fn failed(error: &Value) -> TurnEnd {
    TurnEnd {
        ok: false,
        subtype: "failed".into(),
        reason: Some(error_kind(error)),
        message: error["message"].as_str().map(String::from),
        ..TurnEnd::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a recording's `out` lines through a session opened as the recording's was.
    fn played(name: &str, opening: Opening) -> (Session, Vec<Frame>, Vec<Value>) {
        played_until(name, opening, |_| false)
    }

    /// `played`, ending after the first line the engine sent that `last` picks.
    fn played_until(
        name: &str,
        opening: Opening,
        last: impl Fn(&Value) -> bool,
    ) -> (Session, Vec<Frame>, Vec<Value>) {
        let path = format!("{}/tests/frames/{name}.jsonl", env!("CARGO_MANIFEST_DIR"));
        let (mut session, first) = Session::open(opening);
        let (mut frames, mut sent) = (Vec::new(), vec![first]);
        for row in std::fs::read_to_string(path).unwrap().lines() {
            let row: Value = serde_json::from_str(row).unwrap();
            if row["dir"] != "out" {
                continue;
            }
            let line: Value = serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
            let read = session.read(&line);
            frames.extend(read.frames);
            sent.extend(read.send);
            if last(&line) {
                break;
            }
        }
        (session, frames, sent)
    }

    fn opening() -> Opening {
        Opening {
            cwd: "gathr-test-data".into(),
            policy: crate::codex::command::policy(crate::agents::Approvals::Auto),
            instructions: Some("briefing".into()),
            ..Opening::default()
        }
    }

    /// A compaction reads once, as its item completes (recorded on a test account, Codex 0.166.0,
    /// compacting by itself); Codex gives no sizes.
    #[test]
    fn a_context_compaction_item_reads_once() {
        let (_, frames, _) = played("compact_codex", opening());
        let compacted: Vec<&Frame> = frames
            .iter()
            .filter(|f| matches!(f, Frame::Compacted { .. }))
            .collect();
        let once = Frame::Compacted {
            trigger: "auto".into(),
            before: None,
            after: None,
        };
        assert_eq!(compacted, [&once]);
    }

    /// Codex 0.166.0 reports a compaction's usage as a call with no input and no output: no call ran,
    /// so the fill stays the last real call's, followed or not by a call after it (#125's review).
    #[test]
    fn a_compactions_empty_usage_is_no_fill() {
        let fills = |frames: Vec<Frame>| -> Vec<i64> {
            frames
                .into_iter()
                .filter_map(|f| match f {
                    Frame::Fill { used, .. } => Some(used),
                    _ => None,
                })
                .collect()
        };
        let (_, frames, _) = played("compact_codex", opening());
        assert_eq!(fills(frames), [14_577, 14_835]);
        let (_, frames, _) = played_until("compact_codex", opening(), |line| {
            line["method"] == "item/completed"
                && line["params"]["item"]["type"] == "contextCompaction"
        });
        assert!(frames.iter().any(|f| matches!(f, Frame::Compacted { .. })));
        assert_eq!(fills(frames), [14_577]);
    }

    /// Each call's prompt (`last.inputTokens`) and the model's window (compaction plan, K1).
    #[test]
    fn token_usage_says_how_full_the_context_is_and_its_window() {
        let (_, frames, _) = played("codex_signed_in", opening());
        let fills: Vec<&Frame> = frames
            .iter()
            .filter(|f| matches!(f, Frame::Fill { .. }))
            .collect();
        assert_eq!(
            fills.last(),
            Some(&&Frame::Fill {
                used: 16_291,
                model: None
            })
        );
        assert!(frames.contains(&Frame::Windows(vec![(None, 475_000)])));
    }

    /// Codex 0.166.0's `thread/start` answered these back as its sandbox's `writableRoots` (0034).
    #[test]
    fn the_agents_folders_open_the_thread_as_writable_roots() {
        let folders = vec!["agent-folder".to_string(), "shared".to_string()];
        let opening = Opening {
            writable: folders.clone(),
            ..opening()
        };
        let (_, _, sent) = played("codex_signed_out", opening);
        let config = &sent[2]["params"]["config"];
        assert_eq!(
            config["sandbox_workspace_write"]["writable_roots"],
            json!(folders),
            "{}",
            sent[2]
        );
    }

    /// The owner's call (2026-11-10): a Codex reviewer must reach GitHub to merge, as Claude agents can.
    #[test]
    fn edits_and_auto_open_the_thread_with_the_network_on_and_ask_without() {
        use crate::agents::Approvals;
        for (approvals, on) in [
            (Approvals::Edits, Some(true)),
            (Approvals::Auto, Some(true)),
            (Approvals::Ask, None),
        ] {
            let opening = Opening {
                policy: crate::codex::command::policy(approvals),
                ..opening()
            };
            let (_, _, sent) = played("codex_signed_out", opening);
            let network = &sent[2]["params"]["config"]["sandbox_workspace_write"]["network_access"];
            assert_eq!(network.as_bool(), on, "{approvals:?}: {}", sent[2]);
        }
    }

    #[test]
    fn a_signed_out_turn_opens_its_thread_then_fails_as_signed_out() {
        let (mut session, frames, sent) = played("codex_signed_out", opening());
        let methods: Vec<_> = sent
            .iter()
            .map(|s| s["method"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(methods, ["initialize", "initialized", "thread/start"]);
        assert_eq!(sent[2]["params"]["approvalsReviewer"], "auto_review");
        assert_eq!(sent[2]["params"]["developerInstructions"], "briefing");
        assert_eq!(session.version.as_deref(), Some("0.166.0"));
        let Some(Frame::Init { session_id, .. }) =
            frames.iter().find(|f| matches!(f, Frame::Init { .. }))
        else {
            panic!("no Init in {frames:?}");
        };
        assert_eq!(session_id, "01a1c2f5-7809-7c93-ba52-0dd1f94afa99");
        assert!(frames.contains(&Frame::Read("Say hi.".into())));
        let retried = frames
            .iter()
            .filter(|f| matches!(f, Frame::EngineError { .. }))
            .count();
        assert_eq!(
            retried, 1,
            "Codex's own retries are only recorded: {frames:?}"
        );
        assert!(frames.iter().any(
            |f| matches!(f, Frame::EngineError { kind, .. } if kind == "authentication_failed")
        ));
        let Some(Frame::TurnEnd(end)) = frames.last() else {
            panic!("the turn didn't end: {frames:?}");
        };
        assert!(!end.ok);
        assert_eq!(end.duration_ms, Some(17763));
        assert!(
            session.interrupt().is_none(),
            "no turn is under way after it ended"
        );
    }

    /// `thinking_codex_live` is hand-written in the shape of real frames (its first row says so):
    /// asked for a summary, Codex starts a reasoning item when the thinking begins and completes
    /// it when it ends. The summary is never read.
    #[test]
    fn a_reasoning_item_under_way_reads_as_a_start_then_an_end_and_none_of_its_summary() {
        let (_, frames, _) = played("thinking_codex_live", opening());
        let kinds = frames.iter().filter_map(|f| match f {
            Frame::Thinking { tokens: None } => Some("thinking"),
            Frame::Thought => Some("thought"),
            Frame::Text(_) | Frame::TextDelta(_) => Some("text"),
            _ => None,
        });
        let expected = ["thinking", "thought", "thinking", "thought", "text"];
        assert_eq!(kinds.collect::<Vec<_>>(), expected);
        assert!(!format!("{frames:?}").contains("made-up"), "{frames:?}");
    }

    #[test]
    fn a_real_signed_in_turn_streams_tools_refusals_and_usage() {
        let (session, frames, _) = played("codex_signed_in", opening());
        assert_eq!(session.version.as_deref(), Some("0.167.0"));
        assert!(frames.contains(&Frame::Text("Assigned task #1 to Moss.".into())));
        let tools: Vec<_> = frames
            .iter()
            .filter_map(|f| match f {
                Frame::ToolStart { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            tools,
            [
                "mcp__gathr__list_tasks",
                "mcp__gathr__read_chat",
                "mcp__gathr__give_task",
                "mcp__gathr__finish_task"
            ]
        );
        let completed: Vec<_> = frames
            .iter()
            .filter_map(|f| match f {
                Frame::ToolEnd { ok, .. } => Some(*ok),
                _ => None,
            })
            .collect();
        assert_eq!(
            completed,
            [true, true, true, false],
            "tool refusal lost: {frames:?}"
        );
        let Some(Frame::TurnEnd(end)) = frames.last() else {
            panic!("no end: {frames:?}")
        };
        assert!(end.ok, "a refused tool failed the whole turn: {end:?}");
        assert_eq!(
            (end.usage.input, end.usage.cache_read, end.usage.output),
            (4578, 74624, 222)
        );
        assert_eq!(end.duration_ms, Some(18713));
    }

    #[test]
    fn a_message_before_the_thread_opens_is_held_then_sent_as_a_turn() {
        let (mut session, _) = Session::open(opening());
        assert!(session.user("first").is_none());
        let answer = |id, result: Value| json!({ "id": id, "result": result });
        let agent = json!({ "userAgent": "gathr/0.166.0 (x)" });
        let read = session.read(&answer(1, agent));
        assert_eq!(read.send[1]["method"], "thread/start");
        let read = session.read(&answer(
            2,
            json!({ "thread": { "id": "t1" }, "model": "m" }),
        ));
        assert_eq!(read.send.len(), 1);
        assert_eq!(read.send[0]["method"], "turn/start");
        assert_eq!(read.send[0]["params"]["input"][0]["text"], "first");
        // Asked for, a summary makes Codex say when its thinking starts and ends (0042).
        assert_eq!(read.send[0]["params"]["summary"], "auto");
        session.read(&json!({ "method": "turn/started", "params": { "turn": { "id": "u1" } } }));
        let steer = session.user("more").unwrap();
        assert_eq!(steer["method"], "turn/steer");
        assert_eq!(steer["params"]["expectedTurnId"], "u1");
        assert_eq!(session.interrupt().unwrap()["params"]["turnId"], "u1");
    }

    #[test]
    fn an_approval_is_answered_by_its_own_id_and_other_asks_are_refused() {
        let (mut session, _) = Session::open(opening());
        let ask = json!({ "id": 7, "method": "item/commandExecution/requestApproval",
            "params": { "command": "git push", "cwd": "w", "reason": "network" } });
        let read = session.read(&ask);
        assert!(
            matches!(&read.frames[0], Frame::ApprovalAsked { request_id, tool, .. } if request_id == "7" && tool == "Shell")
        );
        assert_eq!(
            session.approval("7", false).unwrap(),
            json!({ "id": 7, "result": { "decision": "decline" } })
        );
        assert!(session.approval("7", true).is_none(), "answered once");
        let other = session
            .read(&json!({ "id": "x", "method": "item/tool/requestUserInput", "params": {} }));
        assert_eq!(other.send[0]["id"], "x");
        assert!(other.send[0]["error"].is_object());
    }

    #[test]
    fn a_turn_counts_only_its_own_tokens() {
        let (mut session, _) = Session::open(opening());
        let usage = |total: i64, last: i64| {
            json!({ "method": "thread/tokenUsage/updated", "params": { "tokenUsage": {
            "total": { "inputTokens": total, "cachedInputTokens": 0, "outputTokens": total },
            "last": { "inputTokens": last, "cachedInputTokens": 0, "outputTokens": last } } } })
        };
        session.read(&usage(100, 100));
        session.read(&json!({ "method": "turn/started", "params": { "turn": { "id": "u" } } }));
        session.read(&usage(130, 30));
        session.read(&usage(150, 20));
        let done = json!({ "method": "turn/completed", "params": { "turn": { "id": "u", "status": "completed" } } });
        let Frame::TurnEnd(end) = &session.read(&done).frames[0] else {
            panic!()
        };
        assert_eq!((end.usage.input, end.usage.output), (50, 50));
        assert!(end.ok);
    }
}

#[cfg(test)]
mod start_tests {
    use super::*;
    fn opened() -> Session {
        let (mut s, _) = Session::open(Opening::default());
        s.read(&json!({"id":1,"result":{"userAgent":"gathr/0.166.0 (test)"}}));
        s
    }
    #[test]
    fn messages_before_the_thread_opens_start_one_turn_then_steer() {
        let mut s = opened();
        assert!(s.user("first").is_none());
        assert!(s.user("second").is_none());
        let opened = s.read(&json!({"id":2,"result":{"thread":{"id":"thread"}}}));
        assert_eq!(
            opened.send.len(),
            1,
            "two turns started before either answered: {:?}",
            opened.send
        );
        let next = s.read(&json!({"id":3,"result":{"turn":{"id":"turn"}}}));
        assert_eq!(next.send[0]["method"], "turn/steer");
        assert_eq!(next.send[0]["params"]["expectedTurnId"], "turn");
    }
    #[test]
    fn stop_before_the_thread_opens_never_starts_the_held_message() {
        let mut s = opened();
        s.user("first");
        assert!(s.interrupt().is_none());
        let read = s.read(&json!({"id":2,"result":{"thread":{"id":"thread"}}}));
        assert!(read.send.is_empty(), "Stop started a turn: {:?}", read.send);
        assert!(
            read.frames.iter().any(
                |f| matches!(f,Frame::TurnEnd(end) if end.reason.as_deref()==Some("interrupted"))
            ),
            "Stop never ended: {:?}",
            read.frames
        );
    }
}

#[cfg(test)]
mod latest_tests {
    use super::*;
    #[test]
    fn a_family_latest_resolves_from_all_pages_before_opening_the_thread() {
        let (mut session, _) = Session::open(Opening {
            model: Some("latest:sol".into()),
            ..Opening::default()
        });
        session.user("hello");
        let read = session.read(&json!({"id":1,"result":{"userAgent":"gathr/0.167.0 (test)"}}));
        assert_eq!(
            read.send[1]["method"], "model/list",
            "latest was sent straight to Codex: {read:?}"
        );
        let read = session.read(&json!({"id":2,"result":{"data":[{"model":"gpt-6-sol","isDefault":true}],"nextCursor":"next"}}));
        assert_eq!(read.send[0]["params"]["cursor"], "next");
        let read = session
            .read(&json!({"id":3,"result":{"data":[{"model":"gpt-6.1-sol"}],"nextCursor":null}}));
        assert_eq!(read.send[0]["method"], "thread/start");
        assert_eq!(read.send[0]["params"]["model"], "gpt-6.1-sol");
        let read = session.read(&json!({"id":4,"result":{"thread":{"id":"thread"}}}));
        assert_eq!(read.send[0]["method"], "turn/start");
        assert_eq!(read.send[0]["params"]["input"][0]["text"], "hello");
    }
}
