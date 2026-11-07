//! Claude Code's stream-json frames: what the driver reads from stdout and writes to stdin
//! (docs/decisions/0001-engine-interface.md).

use serde_json::{Value, json};

/// What one line of Claude Code's output means to the driver.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// The session is up: its id, model and the tools it offers.
    Init {
        session_id: String,
        model: String,
        tools: Vec<String>,
    },
    /// The provider has begun a reply (`message_start`): the login worked (0016's gate).
    Replied,
    /// Text as it's written (`--include-partial-messages`).
    TextDelta(String),
    /// A whole text block of a finished model call.
    Text(String),
    ToolStart {
        id: String,
        name: String,
    },
    ToolEnd {
        id: String,
        ok: bool,
    },
    /// A message it has read, said back (`--replay-user-messages`): one sent mid-turn is taken.
    Read(String),
    /// It asks to run a tool (`--permission-prompt-tool stdio`).
    ApprovalAsked {
        request_id: String,
        tool: String,
        input: Value,
    },
    /// The answer to a control request the driver sent.
    ControlAnswer {
        request_id: String,
        ok: bool,
    },
    /// The CLI speaking for itself because it can't run the turn ("Not logged in").
    EngineError {
        kind: String,
        message: String,
    },
    TurnEnd(TurnEnd),
    /// A sub-agent's frame, under the Agent or Skill call that started it (`parent_tool_use_id`).
    /// Never the agent's own words or tools.
    Sub {
        parent: String,
        frame: Box<Frame>,
    },
    /// Anything else, by its type: recorded raw, not acted on.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TurnEnd {
    pub ok: bool,
    /// `success`, `error_during_execution`, `error_max_turns`, …
    pub subtype: String,
    /// Why it ended: `completed`, `api_error`, `interrupted`, …, when the engine says.
    pub reason: Option<String>,
    pub message: Option<String>,
    pub usage: Usage,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<i64>,
}

/// Token totals for the turn as the engine reports them, never estimated.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

/// The frame's `type` (and `subtype` for system and control frames), for the raw record.
pub fn kind(value: &Value) -> String {
    let get = |key| value.get(key).and_then(Value::as_str);
    let inner = value
        .get("event")
        .and_then(|e| e.get("type"))
        .and_then(Value::as_str);
    let request = value
        .get("request")
        .and_then(|r| r.get("subtype"))
        .and_then(Value::as_str);
    match (get("type"), get("subtype").or(inner).or(request)) {
        (Some(t), Some(sub)) => format!("{t}/{sub}"),
        (Some(t), None) => t.to_string(),
        (None, _) => "unknown".into(),
    }
}

/// Reads one parsed output line.
pub fn read(value: &Value) -> Vec<Frame> {
    let frames = read_own(value);
    // Null for the main conversation; a sub-agent's frames carry the id of the call that
    // started it. Stream events are documented as main-session only, but are checked the same way.
    match value.get("parent_tool_use_id").and_then(Value::as_str) {
        Some(parent) => frames
            .into_iter()
            .map(|frame| Frame::Sub {
                parent: parent.to_string(),
                frame: Box::new(frame),
            })
            .collect(),
        None => frames,
    }
}

fn read_own(value: &Value) -> Vec<Frame> {
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    match value.get("type").and_then(Value::as_str).unwrap_or("") {
        "system" if text(value, "subtype") == "init" => vec![Frame::Init {
            session_id: text(value, "session_id"),
            model: text(value, "model"),
            tools: strings(value.get("tools")),
        }],
        "stream_event" => delta(value).into_iter().collect(),
        "assistant" => assistant(value),
        "user" if value.get("isReplay") == Some(&Value::Bool(true)) => {
            let content = &value["message"]["content"];
            vec![Frame::Read(
                content
                    .as_str()
                    .map_or_else(|| joined_text(content), String::from),
            )]
        }
        "user" => blocks(&value["message"]["content"])
            .filter(|b| b["type"] == "tool_result")
            .map(|b| Frame::ToolEnd {
                id: text(b, "tool_use_id"),
                ok: b.get("is_error") != Some(&Value::Bool(true)),
            })
            .collect(),
        "control_request" if value["request"]["subtype"] == "can_use_tool" => {
            vec![Frame::ApprovalAsked {
                request_id: text(value, "request_id"),
                tool: text(&value["request"], "tool_name"),
                input: value["request"]["input"].clone(),
            }]
        }
        "control_response" => vec![Frame::ControlAnswer {
            request_id: text(&value["response"], "request_id"),
            ok: value["response"]["subtype"] == "success",
        }],
        "result" => vec![Frame::TurnEnd(turn_end(value))],
        other => vec![Frame::Other(other.to_string())],
    }
}

fn delta(value: &Value) -> Option<Frame> {
    let event = &value["event"];
    let delta = &event["delta"];
    if event["type"] == "message_start" {
        return Some(Frame::Replied);
    }
    (event["type"] == "content_block_delta" && delta["type"] == "text_delta")
        .then(|| Frame::TextDelta(delta["text"].as_str().unwrap_or("").to_string()))
}

fn assistant(value: &Value) -> Vec<Frame> {
    let content = &value["message"]["content"];
    if let Some(kind) = value.get("error").and_then(Value::as_str) {
        return vec![Frame::EngineError {
            kind: kind.to_string(),
            message: joined_text(content),
        }];
    }
    blocks(content)
        .filter_map(|b| match b["type"].as_str() {
            Some("text") => Some(Frame::Text(b["text"].as_str().unwrap_or("").to_string())),
            Some("tool_use") => Some(Frame::ToolStart {
                id: b["id"].as_str().unwrap_or("").to_string(),
                name: b["name"].as_str().unwrap_or("").to_string(),
            }),
            _ => None,
        })
        .collect()
}

fn turn_end(value: &Value) -> TurnEnd {
    let usage = &value["usage"];
    let n = |key: &str| usage.get(key).and_then(Value::as_i64).unwrap_or(0);
    TurnEnd {
        ok: value.get("is_error") != Some(&Value::Bool(true)),
        subtype: value["subtype"].as_str().unwrap_or("").to_string(),
        reason: value["terminal_reason"].as_str().map(String::from),
        message: value["result"].as_str().map(String::from),
        usage: Usage {
            input: n("input_tokens"),
            output: n("output_tokens"),
            cache_read: n("cache_read_input_tokens"),
            cache_write: n("cache_creation_input_tokens"),
        },
        cost_usd: value["total_cost_usd"].as_f64(),
        duration_ms: value["duration_ms"].as_i64(),
    }
}

fn blocks(content: &Value) -> impl Iterator<Item = &Value> {
    content.as_array().into_iter().flatten()
}

fn joined_text(content: &Value) -> String {
    blocks(content)
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// A message from the person, as a stdin line.
pub fn user(text: &str) -> Value {
    json!({
        "type": "user",
        "message": { "role": "user", "content": text },
        "parent_tool_use_id": null,
        "session_id": "default",
    })
}

/// Stop: the engine's own interrupt, never a kill.
pub fn interrupt(request_id: &str) -> Value {
    json!({ "type": "control_request", "request_id": request_id, "request": { "subtype": "interrupt" } })
}

/// The person's answer to `ApprovalAsked`.
pub fn approval(request_id: &str, allow: bool, input: &Value) -> Value {
    let decision = match allow {
        true => json!({ "behavior": "allow", "updatedInput": input }),
        false => json!({ "behavior": "deny", "message": "The person declined." }),
    };
    json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": request_id, "response": decision },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::command::{DENIED_TOOLS, KNOWN_TOOLS};

    /// The `out` frames of a recording in `tests/frames`, read.
    pub fn replayed(name: &str) -> Vec<Frame> {
        let path = format!("{}/tests/frames/{name}.jsonl", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap();
        text.lines()
            .map(|row| serde_json::from_str::<Value>(row).unwrap())
            .filter(|row| row["dir"] == "out")
            .flat_map(|row| read(&serde_json::from_str(row["line"].as_str().unwrap()).unwrap()))
            .collect()
    }

    #[test]
    fn a_signed_out_engine_reads_as_an_error_then_a_failed_end() {
        let frames = replayed("signed_out");
        let error = frames
            .iter()
            .find(|f| matches!(f, Frame::EngineError { .. }));
        assert_eq!(
            error,
            Some(&Frame::EngineError {
                kind: "authentication_failed".into(),
                message: "Not logged in · Please run /login".into()
            })
        );
        let Some(Frame::TurnEnd(end)) = frames.last() else {
            panic!("the last frame isn't the turn's end: {frames:?}");
        };
        assert!(!end.ok);
        assert_eq!(end.reason.as_deref(), Some("api_error"));
        assert!(
            frames.contains(&Frame::Read("Say hi".into())),
            "the echo of the message read is missing"
        );
    }

    /// The macOS gate lets the next start go on this, so it must come before the reply's words,
    /// and a signed-out run, which never reached the provider, must have none (0016).
    #[test]
    fn the_providers_first_reply_reads_before_its_text() {
        let frames = replayed("approval");
        let replied = frames.iter().position(|f| *f == Frame::Replied);
        let text = frames
            .iter()
            .position(|f| matches!(f, Frame::TextDelta(_) | Frame::Text(_)));
        assert!(
            replied.is_some() && replied < text,
            "message_start at {replied:?}, first text at {text:?}"
        );
        assert!(!replayed("signed_out").contains(&Frame::Replied));
    }

    /// A sub-agent's words and tools are its own, tied to the Agent call, never the agent's.
    #[test]
    fn a_sub_agents_frames_are_kept_apart_under_its_agent_call() {
        let frames = replayed("subagent");
        let Some(agent_call) = frames.iter().find_map(|f| match f {
            Frame::ToolStart { id, name } if name == "Agent" => Some(id.clone()),
            _ => None,
        }) else {
            panic!("no Agent call of the agent's own: {frames:?}");
        };
        let (sub, own): (Vec<_>, Vec<_>) =
            frames.iter().partition(|f| matches!(f, Frame::Sub { .. }));
        let sub_text: Vec<&str> = sub
            .iter()
            .filter_map(|f| match f {
                Frame::Sub { parent, frame } => {
                    assert_eq!(
                        parent, &agent_call,
                        "a sub-agent frame under another call: {f:?}"
                    );
                    match frame.as_ref() {
                        Frame::Text(t) => Some(t.as_str()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();
        assert!(
            !sub_text.is_empty(),
            "the sub-agent's text is missing: {sub:?}"
        );
        let own_text: Vec<&str> = own
            .iter()
            .filter_map(|f| match f {
                Frame::Text(t) | Frame::TextDelta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        for text in &sub_text {
            assert!(
                !own_text.contains(text),
                "the sub-agent's {text:?} reads as the agent's own"
            );
        }
        assert!(
            own.contains(&&Frame::ToolEnd {
                id: agent_call.clone(),
                ok: true
            }),
            "the Agent call's own end is missing"
        );
        assert!(
            !own.iter().any(|f| matches!(f, Frame::ToolStart { id, .. } | Frame::ToolEnd { id, .. } if *id != agent_call)),
            "a sub-agent's tool reads as the agent's own: {own:?}"
        );
    }

    /// 0001: fails when a denied tool is still offered, or the engine offers one nobody looked at.
    #[test]
    fn the_engine_offers_no_denied_tool_and_none_unlooked_at() {
        let Some(Frame::Init { tools, .. }) = replayed("signed_out").into_iter().next() else {
            panic!("the recording doesn't start with init");
        };
        for tool in &tools {
            assert!(
                !DENIED_TOOLS.iter().any(|(d, _)| d == tool),
                "{tool} is denied but offered"
            );
            assert!(
                KNOWN_TOOLS.contains(&tool.as_str()),
                "{tool} is new: look at it, then add it to KNOWN_TOOLS"
            );
        }
        assert!(
            tools.iter().any(|t| t == "TaskCreate"),
            "the task tools are off: {tools:?}"
        );
    }
}
