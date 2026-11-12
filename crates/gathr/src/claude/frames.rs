//! Claude Code's stream-json frames: what the driver reads from stdout and writes to stdin
//! (docs/decisions/0001-engine-interface.md).

use serde_json::{Value, json};

use crate::steps::{self, StepInput};

/// What one line of Claude Code's output means to the driver.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// The session is up: its id, model, the tools it offers and the permission mode it runs in.
    Init {
        session_id: String,
        model: String,
        tools: Vec<String>,
        permission_mode: String,
    },
    /// The provider has begun a reply (`message_start`): the login worked (0016's gate).
    Replied,
    /// Text as it's written (`--include-partial-messages`).
    TextDelta(String),
    /// A whole text block of a finished model call.
    Text(String),
    /// A tool call began, with what its step keeps of its input (0041).
    ToolStart {
        id: String,
        name: String,
        input: Option<StepInput>,
    },
    /// A tool call ended; `input` when the engine says what it ran only now (Codex's web search).
    ToolEnd {
        id: String,
        ok: bool,
        input: Option<StepInput>,
    },
    /// A tool call arrived without the field its step is read from: the engine's protocol changed.
    ToolShape {
        tool: String,
        field: &'static str,
    },
    /// It is thinking (0042): a thinking block began, or its running count of tokens moved. Sent
    /// more than once in a thought; `tokens` is the engine's estimate so far, when it gives one.
    Thinking {
        tokens: Option<i64>,
    },
    /// A thought ended. Claude Code: its block is whole. Codex: the only sign it gives of one.
    Thought,
    /// A message it has read, said back (`--replay-user-messages`): one sent mid-turn is taken.
    Read(String),
    /// It asks to run a tool (`--permission-prompt-tool stdio`), with why it asks when it says.
    ApprovalAsked {
        request_id: String,
        tool: String,
        input: Value,
        why: Option<String>,
    },
    /// It refused a tool call without asking, and what decided it: `classifier` is auto mode's
    /// own check (0029).
    Blocked {
        tool: String,
        by: String,
        why: String,
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
    /// Every command running in the background now (`background_tasks_changed`): id and what it is.
    Background(Vec<(String, String)>),
    /// Where the account stands on usage (`rate_limit_event`): `rejected` turns turns away until
    /// `resets_at` (Unix seconds); `kind` names the limit (`five_hour`, `seven_day`).
    Limit {
        status: String,
        resets_at: Option<i64>,
        kind: Option<String>,
    },
    /// A sub-agent's frame, under the Agent or Skill call that started it (`parent_tool_use_id`).
    /// Never the agent's own words or tools.
    Sub {
        parent: String,
        frame: Box<Frame>,
    },
    /// How full the context is after a model call: its prompt in tokens, cached and cache-written
    /// ones included, and the model (Claude names it; Codex doesn't). The last call's, never a sum.
    Fill {
        used: i64,
        model: Option<String>,
    },
    /// Each model's context window, as the engine reports it (never a table of ours).
    Windows(Vec<(Option<String>, i64)>),
    /// The engine compacted the conversation (`manual` or `auto`), with its size before and after
    /// when the engine says (Claude Code does; Codex doesn't).
    Compacted {
        trigger: String,
        before: Option<i64>,
        after: Option<i64>,
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
    /// The result answers a notice about a background command (`origin.kind`), not a message.
    pub notice: bool,
}

/// Token totals for the turn as the engine reports them, never estimated.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
    // Null for the main conversation; a sub-agent's frames carry the id of the call that
    // started it. Stream events are documented as main-session only, but are checked the same way.
    let parent = value.get("parent_tool_use_id").and_then(Value::as_str);
    let frames = read_own(value, parent.is_none());
    match parent {
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

fn read_own(value: &Value, own: bool) -> Vec<Frame> {
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    match value.get("type").and_then(Value::as_str).unwrap_or("") {
        "system" if text(value, "subtype") == "init" => vec![Frame::Init {
            session_id: text(value, "session_id"),
            model: text(value, "model"),
            tools: strings(value.get("tools")),
            permission_mode: text(value, "permissionMode"),
        }],
        "system" if text(value, "subtype") == "compact_boundary" => {
            let meta = &value["compact_metadata"];
            vec![Frame::Compacted {
                trigger: meta["trigger"].as_str().unwrap_or("auto").to_string(),
                before: meta["pre_tokens"].as_i64(),
                after: meta["post_tokens"].as_i64(),
            }]
        }
        "system" if text(value, "subtype") == "permission_denied" => vec![Frame::Blocked {
            tool: text(value, "tool_name"),
            by: text(value, "decision_reason_type"),
            why: text(value, "decision_reason"),
        }],
        "system" if text(value, "subtype") == "background_tasks_changed" => {
            let tasks = blocks(&value["tasks"]);
            let tasks = tasks.map(|t| (text(t, "task_id"), text(t, "description")));
            vec![Frame::Background(tasks.collect())]
        }
        // Documented from Claude Code 2.1.153: sent while a thinking block, a redacted one too,
        // is produced.
        "system" if text(value, "subtype") == "thinking_tokens" => vec![Frame::Thinking {
            tokens: value["estimated_tokens"].as_i64(),
        }],
        "rate_limit_event" => {
            let info = &value["rate_limit_info"];
            vec![Frame::Limit {
                status: text(info, "status"),
                resets_at: info["resetsAt"].as_i64(),
                kind: info["rateLimitType"].as_str().map(String::from),
            }]
        }
        "stream_event" => delta(value).into_iter().collect(),
        "assistant" => assistant(value, own),
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
                input: None,
            })
            .collect(),
        "control_request" if value["request"]["subtype"] == "can_use_tool" => {
            vec![Frame::ApprovalAsked {
                request_id: text(value, "request_id"),
                tool: text(&value["request"], "tool_name"),
                input: value["request"]["input"].clone(),
                why: value["request"]["decision_reason"]
                    .as_str()
                    .map(String::from),
            }]
        }
        "control_response" => vec![Frame::ControlAnswer {
            request_id: text(&value["response"], "request_id"),
            ok: value["response"]["subtype"] == "success",
        }],
        "result" => {
            let windows = value["modelUsage"].as_object().into_iter().flatten();
            let windows: Vec<_> = windows
                .filter_map(|(model, u)| Some((Some(model.clone()), u["contextWindow"].as_i64()?)))
                .collect();
            let mut frames = vec![Frame::TurnEnd(turn_end(value))];
            if !windows.is_empty() {
                frames.insert(0, Frame::Windows(windows));
            }
            frames
        }
        other => vec![Frame::Other(other.to_string())],
    }
}

fn delta(value: &Value) -> Option<Frame> {
    let event = &value["event"];
    let delta = &event["delta"];
    if event["type"] == "message_start" {
        return Some(Frame::Replied);
    }
    if event["type"] == "content_block_start" && thinks(&event["content_block"]) {
        return Some(Frame::Thinking { tokens: None });
    }
    (event["type"] == "content_block_delta" && delta["type"] == "text_delta")
        .then(|| Frame::TextDelta(delta["text"].as_str().unwrap_or("").to_string()))
}

fn assistant(value: &Value, own: bool) -> Vec<Frame> {
    let content = &value["message"]["content"];
    if let Some(kind) = value.get("error").and_then(Value::as_str) {
        return vec![Frame::EngineError {
            kind: kind.to_string(),
            message: joined_text(content),
        }];
    }
    let usage = &value["message"]["usage"];
    let n = |key: &str| usage[key].as_i64().unwrap_or(0);
    let fill = usage.is_object().then(|| Frame::Fill {
        used: n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"),
        model: value["message"]["model"].as_str().map(String::from),
    });
    let said = blocks(content).flat_map(|b| match b["type"].as_str() {
        Some("text") => vec![Frame::Text(b["text"].as_str().unwrap_or("").to_string())],
        Some("tool_use") => {
            let name = b["name"].as_str().unwrap_or("").to_string();
            let input = step(&name, &b["input"], own);
            let missing = input.as_ref().err().map(|field| Frame::ToolShape {
                tool: name.clone(),
                field,
            });
            let start = Frame::ToolStart {
                id: b["id"].as_str().unwrap_or("").to_string(),
                name,
                input: input.unwrap_or(None),
            };
            std::iter::once(start).chain(missing).collect()
        }
        // The block arrives empty, with a signature: only that it thought is read (0042).
        _ if thinks(b) => vec![Frame::Thought],
        _ => Vec::new(),
    });
    said.chain(fill).collect()
}

fn thinks(block: &Value) -> bool {
    matches!(
        block["type"].as_str(),
        Some("thinking" | "redacted_thinking")
    )
}

/// What a tool call's step keeps of its input (0041), or the name of the field it's read from
/// when the call lacks it. Only the tools whose input was seen in real frames; Gathr's own tools,
/// other MCP tools and anything not listed keep nothing. A sub-agent's steps aren't kept or shown
/// (`own` is false): its call is only checked for the field.
fn step(tool: &str, input: &Value, own: bool) -> Result<Option<StepInput>, &'static str> {
    let (field, title) = match tool {
        "Bash" | "PowerShell" => ("command", input["description"].as_str()),
        "Read" | "Write" | "Edit" => ("file_path", None),
        "Grep" => ("pattern", None),
        "WebFetch" => ("url", None),
        "Agent" => ("description", None),
        _ => return Ok(None),
    };
    let text = input[field].as_str().ok_or(field)?;
    Ok(own.then(|| steps::input(text, title)).flatten())
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
        notice: value["origin"]["kind"] == "task-notification",
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

/// Ends a command the engine runs in the background, by its id: the engine's own stop, never a
/// kill by process (checked live on 2.1.319: it answers `task_updated` with `killed`).
pub fn stop_task(request_id: &str, task: &str) -> Value {
    json!({ "type": "control_request", "request_id": request_id,
        "request": { "subtype": "stop_task", "task_id": task } })
}

/// The handshake that lists the engine's models and checks its switches, without a turn (0025).
pub fn initialize(request_id: &str) -> Value {
    json!({ "type": "control_request", "request_id": request_id, "request": { "subtype": "initialize" } })
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

    /// How full the agent's context is: each main-session call's prompt (input, cache read and cache
    /// write), and the window of its model from the turn's result (compaction plan, K1).
    #[test]
    fn each_call_says_how_full_the_context_is_and_the_result_gives_its_window() {
        let frames = replayed("signed_in");
        let fill = Frame::Fill {
            used: 2 + 23_627 + 1_711,
            model: Some("claude-opus-5-5".into()),
        };
        assert!(frames.contains(&fill), "{frames:?}");
        let windows = Frame::Windows(vec![(Some("claude-opus-5-5".into()), 1_000_000)]);
        assert!(frames.contains(&windows), "{frames:?}");
    }

    /// A compaction, read once from its boundary, with the sizes before and after (recorded with
    /// `/compact` on a test account, Claude Code 2.1.318).
    #[test]
    fn a_compaction_reads_once_with_its_sizes() {
        let frames = replayed("compact_claude");
        let compacted: Vec<&Frame> = frames
            .iter()
            .filter(|f| matches!(f, Frame::Compacted { .. }))
            .collect();
        let once = Frame::Compacted {
            trigger: "manual".into(),
            before: Some(23_042),
            after: Some(3_611),
        };
        assert_eq!(compacted, [&once]);
    }

    /// A sub-agent's calls fill its own context, never the agent's.
    #[test]
    fn a_sub_agents_calls_never_count_as_the_agents_fill() {
        let frames = replayed("subagent");
        let own: Vec<i64> = frames
            .iter()
            .filter_map(|f| match f {
                Frame::Fill { used, .. } => Some(*used),
                _ => None,
            })
            .collect();
        assert_eq!(own, [34_315, 34_758]);
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
            Frame::ToolStart { id, name, .. } if name == "Agent" => Some(id.clone()),
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
                ok: true,
                input: None
            }),
            "the Agent call's own end is missing"
        );
        assert!(
            !own.iter().any(|f| matches!(f, Frame::ToolStart { id, .. } | Frame::ToolEnd { id, .. } if *id != agent_call)),
            "a sub-agent's tool reads as the agent's own: {own:?}"
        );
    }

    /// Each of the agent's own tool calls in a recording: its name, and its step's text and title.
    fn steps_of(name: &str) -> Vec<(String, Option<String>, Option<String>)> {
        let own = replayed(name).into_iter().filter_map(|f| match f {
            Frame::ToolStart { name, input, .. } => {
                let (text, title) = input.map_or((None, None), |i| (Some(i.text), i.title));
                Some((name, text, title))
            }
            _ => None,
        });
        own.collect()
    }

    fn kept(
        name: &str,
        text: Option<&str>,
        title: Option<&str>,
    ) -> (String, Option<String>, Option<String>) {
        (name.into(), text.map(String::from), title.map(String::from))
    }

    /// 0041, in real recordings: a command keeps its first line and the engine's own words for it,
    /// a written file its path and never its content, a sub-agent what it was asked to do.
    #[test]
    fn a_recorded_tool_call_keeps_what_it_ran() {
        let ping = kept(
            "Bash",
            Some("ping -n 25 127.0.0.1"),
            Some("Run first ping in background"),
        );
        assert_eq!(steps_of("background"), [ping]);
        let path = "gathr-test-data\\chats\\01M6VP8XTYX9PQ6YJPQMFQ6SHQ\\note.txt";
        assert_eq!(steps_of("approval"), [kept("Write", Some(path), None)]);
        assert_eq!(
            steps_of("subagent"),
            [kept("Agent", Some("Read a.txt contents"), None)]
        );
    }

    /// A sub-agent's steps aren't kept or shown yet (G24 keeps them out of the agent's own): its
    /// Read carries no input, and a field it lacks is still found, under its Agent call.
    #[test]
    fn a_sub_agents_tool_keeps_nothing_and_still_says_when_its_field_is_gone() {
        let subs = |frames: Vec<Frame>| -> Vec<Frame> {
            let subs = frames.into_iter().filter_map(|f| match f {
                Frame::Sub { frame, .. } => Some(*frame),
                _ => None,
            });
            subs.collect()
        };
        let recorded = subs(replayed("subagent"));
        let read = recorded.iter().find_map(|f| match f {
            Frame::ToolStart { name, input, .. } if name == "Read" => Some(input.clone()),
            _ => None,
        });
        assert_eq!(read, Some(None), "{recorded:?}");
        let shape = Frame::ToolShape {
            tool: "Grep".into(),
            field: "pattern",
        };
        let changed = subs(replayed("steps_shape_changed"));
        assert!(changed.contains(&shape), "{changed:?}");
    }

    /// `steps_claude` is hand-written in the shape of the owner's live frames (its first row says
    /// so): each listed tool keeps its own field, Write never its content, and an MCP call, a
    /// sub-agent's own tool and a tool nobody listed keep nothing.
    #[test]
    fn each_claude_tool_keeps_its_own_field() {
        let curl = "curl --token=[redacted:flag] -H \"Authorization: Bearer [redacted:bearer]\" \
                    https://example.test/api";
        let expected = [
            kept(
                "Bash",
                Some("git status --short"),
                Some("Show what changed"),
            ),
            kept("PowerShell", Some("cargo fmt --all --check…"), None),
            kept("Read", Some("src/lib.rs"), None),
            kept("Edit", Some("src/lib.rs"), None),
            kept("Write", Some("notes/plan.md"), None),
            kept("Grep", Some("fn main"), None),
            kept("WebFetch", Some("https://example.test/docs/steps"), None),
            kept("Bash", Some(curl), None),
            kept("mcp__gathr__list_tasks", None, None),
            kept("Agent", Some("Find the tests"), None),
            kept("Monitor", None, None),
        ];
        assert_eq!(steps_of("steps_claude"), expected);
        let frames = replayed("steps_claude");
        let missing = frames.iter().any(|f| matches!(f, Frame::ToolShape { .. }));
        assert!(!missing, "a field read as missing: {frames:?}");
    }

    /// A listed tool without its field keeps nothing and says which field is gone (0019).
    #[test]
    fn a_listed_tool_without_its_field_reads_as_a_changed_shape() {
        let frames = replayed("steps_shape_changed");
        let agent = kept("Agent", Some("Find the tests"), None);
        assert_eq!(
            steps_of("steps_shape_changed"),
            [kept("Edit", None, None), agent]
        );
        let shape = Frame::ToolShape {
            tool: "Edit".into(),
            field: "file_path",
        };
        assert!(frames.contains(&shape), "{frames:?}");
    }

    /// 0029: lines from Claude Code 2.1.319 in auto mode (message cut short): a block by its
    /// own check comes as `permission_denied`, never as a question, and a question from Gathr's
    /// hook carries the hook's reason.
    #[test]
    fn auto_modes_blocks_and_gathrs_asks_read_as_what_they_are() {
        let blocked = json!({ "type": "system", "subtype": "permission_denied",
            "tool_name": "Bash", "tool_use_id": "toolu_01", "decision_reason_type": "classifier",
            "decision_reason": "[Data Exfiltration]",
            "message": "Permission for this action was denied by the Claude Code auto mode classifier." });
        let by = Frame::Blocked {
            tool: "Bash".into(),
            by: "classifier".into(),
            why: "[Data Exfiltration]".into(),
        };
        assert_eq!(read(&blocked), [by]);
        let asked = json!({ "type": "control_request", "request_id": "r1", "request": {
            "subtype": "can_use_tool", "tool_name": "Write", "input": { "file_path": "x" },
            "decision_reason": "Gathr asks because it pushes", "decision_reason_type": "hook" } });
        let [Frame::ApprovalAsked { why, .. }] = &read(&asked)[..] else {
            panic!("not an approval: {:?}", read(&asked));
        };
        assert_eq!(why.as_deref(), Some("Gathr asks because it pushes"));
        let init = json!({ "type": "system", "subtype": "init", "permissionMode": "auto" });
        let [
            Frame::Init {
                permission_mode, ..
            },
        ] = &read(&init)[..]
        else {
            panic!("not init");
        };
        assert_eq!(permission_mode, "auto");
    }
    /// 0001: fails when a denied tool is still offered, or the engine offers one nobody looked at.
    #[test]
    fn the_engine_offers_no_denied_tool_and_none_unlooked_at() {
        // Signed out and signed in: a claude.ai account adds tools of its own.
        for recording in ["signed_out", "signed_in"] {
            let Some(Frame::Init { tools, .. }) = replayed(recording).into_iter().next() else {
                panic!("{recording} doesn't start with init");
            };
            for tool in &tools {
                assert!(
                    !DENIED_TOOLS.iter().any(|(d, _)| d == tool),
                    "{recording}: {tool} is denied but offered"
                );
                assert!(
                    KNOWN_TOOLS.contains(&tool.as_str()),
                    "{recording}: {tool} is new: look at it, then add it to KNOWN_TOOLS"
                );
            }
            assert!(
                tools.iter().any(|t| t == "TaskCreate"),
                "{recording}: the task tools are off: {tools:?}"
            );
        }
    }

    /// 0024: signed in to a claude.ai account with plugins of its own, the session loads only
    /// Claude Code's built-in plugins, no skills or commands, and runs no hook.
    #[test]
    fn a_signed_in_session_gets_nothing_from_the_account() {
        let path = format!(
            "{}/tests/frames/signed_in.jsonl",
            env!("CARGO_MANIFEST_DIR")
        );
        let frames: Vec<Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|row| serde_json::from_str::<Value>(row).unwrap())
            .filter(|row| row["dir"] == "out")
            .map(|row| serde_json::from_str(row["line"].as_str().unwrap()).unwrap())
            .collect();
        let init = frames
            .iter()
            .find(|f| f["subtype"] == "init")
            .expect("no init in the recording");
        for plugin in init["plugins"].as_array().unwrap() {
            let source = plugin["source"].as_str().unwrap_or_default();
            assert!(
                source.ends_with("@builtin"),
                "a plugin from outside Gathr loaded: {plugin}"
            );
        }
        for list in ["skills", "slash_commands"] {
            assert_eq!(init[list], json!([]), "{list} from outside Gathr loaded");
        }
        let hooks = frames
            .iter()
            .filter(|f| {
                f["subtype"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("hook_"))
            })
            .count();
        assert_eq!(hooks, 0, "a hook ran");
    }

    /// The frames `kinds` picks, in order.
    fn of(frames: &[Frame], kinds: impl Fn(&Frame) -> bool) -> Vec<&Frame> {
        frames.iter().filter(|f| kinds(f)).collect()
    }

    /// 0042, recorded on Opus 5.5: the thinking block starts, then arrives whole (and empty),
    /// before the reply's text.
    #[test]
    fn a_thought_reads_as_its_start_then_its_end_before_the_text() {
        let frames = replayed("steered_after_turn");
        let thought = of(&frames, |f| {
            matches!(f, Frame::Thinking { .. } | Frame::Thought | Frame::Text(_))
        });
        let start = Frame::Thinking { tokens: None };
        assert_eq!(thought[..2], [&start, &Frame::Thought]);
        assert!(matches!(thought[2], Frame::Text(_)), "{thought:?}");
        assert_eq!(thought.len(), 4, "one thought, two replies: {thought:?}");
    }

    /// Recorded on Haiku 5.5: the running count while it thinks, 50 then 204, then the block.
    #[test]
    fn a_thought_carries_the_engines_last_count_of_its_tokens() {
        let frames = replayed("background");
        let thought = of(&frames, |f| {
            matches!(f, Frame::Thinking { .. } | Frame::Thought)
        });
        let first = Frame::Thinking { tokens: Some(50) };
        let last = Frame::Thinking { tokens: Some(204) };
        assert_eq!(thought, [&first, &last, &Frame::Thought]);
    }

    /// Hand-written in the recorded shape, under an Agent call: a sub-agent's thinking, a redacted
    /// block too, is never the agent's own (G24).
    #[test]
    fn a_sub_agents_thinking_is_never_the_agents() {
        let block = json!({ "type": "redacted_thinking", "data": "made-up" });
        let lines = [
            json!({ "type": "system", "subtype": "thinking_tokens", "estimated_tokens": 12,
                "parent_tool_use_id": "toolu_agent" }),
            json!({ "type": "stream_event", "parent_tool_use_id": "toolu_agent",
                "event": { "type": "content_block_start", "content_block": block } }),
            json!({ "type": "assistant", "parent_tool_use_id": "toolu_agent",
                "message": { "content": [block] } }),
        ];
        let frames: Vec<Frame> = lines.iter().flat_map(read).collect();
        let inner: Vec<&Frame> = frames
            .iter()
            .map(|f| match f {
                Frame::Sub { parent, frame } if parent == "toolu_agent" => &**frame,
                own => panic!("read as the agent's own: {own:?}"),
            })
            .collect();
        let counted = Frame::Thinking { tokens: Some(12) };
        let start = Frame::Thinking { tokens: None };
        assert_eq!(inner, [&counted, &start, &Frame::Thought]);
    }
}
