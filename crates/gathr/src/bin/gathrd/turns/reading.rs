//! Reading the engine's output: each line recorded, then read as frames that move the turn on.

use gathr::claude::command;
use gathr::claude::engine::Line;
use gathr::claude::frames::{Frame, TurnEnd};
use gathr::event::Event;
use gathr::ids::{now_us, span_id};
use gathr::steps::{StepInput, StepResult};
use gathr::store;
use serde_json::{Value, json};

use super::{OpenTool, Queued, Task, login_stamp};

const EMPTY: &str = "the engine ended the turn with no reply and no tool call";

/// A tool message's body: `input` only when the step kept what it ran (0041).
fn tool_body(
    tool: &str,
    status: &str,
    duration_ms: Option<i64>,
    input: Option<&StepInput>,
) -> Value {
    let mut body = json!({ "tool": tool, "status": status });
    if let Some(ms) = duration_ms {
        body["duration_ms"] = ms.into();
    }
    if let Some(input) = input {
        body["input"] = json!(input);
    }
    body
}

impl Task {
    pub(super) async fn on_line(&mut self, line: Line) {
        let pid = self.engine.as_ref().map_or(0, |e| e.pid);
        let turn = self.turn.as_ref().map(|t| t.ids.turn.clone());
        let (direction, text) = match &line {
            Line::Out(text) => ("out", text),
            Line::Err(text) => ("err", text),
        };
        // Written, redacted, before it's acted on (0011). From a stream fragment on, lines wait
        // for the block's end, so a secret split across fragments is redacted whole.
        let parsed = (direction == "out")
            .then(|| serde_json::from_str::<Value>(text).ok())
            .flatten();
        let fragment = parsed
            .as_ref()
            .is_some_and(|v| gathr::redact::stream::fragment(v).is_some());
        if fragment || !self.held_raw.is_empty() {
            self.held_raw
                .push((turn.clone(), pid, direction, text.clone()));
            if parsed
                .as_ref()
                .is_some_and(gathr::redact::stream::ends_block)
            {
                self.flush_raw().await;
            }
        } else {
            self.raw(turn.as_deref(), pid, direction, text).await;
        }
        let Line::Out(text) = line else {
            self.record(Event::point("gathr.engine.stderr").attr("line", text.clone()));
            if !text.trim().is_empty() {
                self.last_stderr = Some(text.clone());
            }
            return;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                let read = self.driver.read(&value);
                let failed = super::driver::Driver::needs_recovery(&read.frames);
                for send in read.send {
                    if let Err(e) = self.write(&send, turn.as_deref()).await {
                        self.record(
                            Event::point("gathr.engine.write")
                                .error("write")
                                .attr("why", e.to_string()),
                        );
                    }
                }
                let failure = read.frames.iter().find_map(|frame| match frame {
                    Frame::EngineError { message, .. } => Some(message.clone()),
                    _ => None,
                });
                // Retire a rejected start or steer before TurnEnd can start another message on
                // the same pending session. Closing recovers every input not acknowledged Read.
                if failed {
                    self.close("the engine rejected its request").await;
                }
                for frame in read.frames {
                    self.on_frame(frame).await;
                }
                if failed {
                    self.turn_ended(TurnEnd {
                        message: failure,
                        ..TurnEnd::default()
                    })
                    .await;
                }
            }
            Err(e) => self.record(
                Event::point("gathr.engine.frame")
                    .error("malformed")
                    .attr("why", e.to_string()),
            ),
        }
    }

    /// The chat the engine runs in.
    fn here(&self) -> String {
        let chat = self.assigned.as_ref().map(|a| a.chat.clone());
        chat.unwrap_or_default()
    }

    pub(super) async fn on_frame(&mut self, frame: Frame) {
        let chat = self.here();
        if matches!(
            frame,
            Frame::Replied | Frame::Text(_) | Frame::ToolStart { .. }
        ) && let Some(turn) = self.turn.as_mut()
        {
            turn.gate.release();
        }
        if matches!(
            frame,
            Frame::TextDelta(_)
                | Frame::Text(_)
                | Frame::ToolStart { .. }
                | Frame::ToolEnd { .. }
                | Frame::Compacted { .. }
        ) && let Some(turn) = self.turn.as_mut()
        {
            turn.progress_at = std::time::Instant::now();
            turn.said = true;
        }
        // A thought is over once words or a tool follow, whether or not its end was said. One
        // whose start is still held was over before anyone could have seen it.
        if matches!(
            frame,
            Frame::TextDelta(_) | Frame::Text(_) | Frame::ToolStart { .. }
        ) && let Some(turn) = self.turn.as_mut()
        {
            turn.thought_began = None;
            if turn.thought.is_some() {
                self.thought("done").await;
            }
        }
        match frame {
            Frame::Replied => {}
            Frame::Init {
                session_id,
                tools,
                permission_mode,
                ..
            } => {
                // With no turn and no message waiting to be read, the engine began one itself.
                if self.turn.is_none() && self.unread.is_empty() && self.closing.is_none() {
                    self.begin_woken_turn().await;
                }
                self.check_tools(&tools);
                self.check_mode(&permission_mode);
                let account = self.assigned.as_ref().map(|a| a.account.clone());
                let (account, agent) = (account.unwrap_or_default(), self.agent.clone());
                let saved = self
                    .chats
                    .writer
                    .call(move |conn| {
                        let agent = agent.as_deref();
                        store::set_engine_session(conn, &chat, agent, &session_id, &account)
                    })
                    .await;
                if let Some(Err(e)) = saved {
                    self.record(
                        Event::point("gathr.engine.session")
                            .error("sqlite")
                            .attr("why", e.to_string()),
                    );
                }
            }
            Frame::TextDelta(text) => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                let at = turn.partial_units;
                turn.partial.push_str(&text);
                turn.partial_units += text.encode_utf16().count();
                let first = !std::mem::replace(&mut turn.streamed, true);
                let (ids, sent) = (turn.ids.clone(), turn.sent_us);
                let delta = json!({ "type": "delta", "turn": ids.turn, "text": text, "at": at });
                self.publish(&ids.chat, delta);
                if first {
                    let mut event = Event::point("gathr.turn.first_token").trace(&ids.trace);
                    event.duration_us = Some(now_us() - sent);
                    self.record(event.chat(&ids.chat).ok());
                }
            }
            Frame::Text(text) => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.partial.clear();
                turn.partial_units = 0;
                let ids = turn.ids.clone();
                self.save_reply(&ids, text, false).await;
            }
            Frame::ToolStart { id, name, input } => self.tool_started(id, name, input).await,
            Frame::ToolEnd {
                id,
                ok,
                input,
                result,
                told,
            } => {
                let status = if ok { "ok" } else { "error" };
                let result = self.kept_result(&id, result, told);
                self.tool_ended(&id, status, input, result).await
            }
            Frame::ToolShape { tool, field } => self.tool_shape(tool, field),
            Frame::Read(text) => {
                let unread = self.unread.iter().position(|(_, t)| *t == text);
                let read = unread.map(|i| self.unread.remove(i));
                // Read inside the turn: part of what it was given (0020), recorded now, so the
                // turn keeps it however it ends.
                if let (Some(_), Some(turn)) = (&read, self.turn.as_ref()) {
                    let chars = text.chars().count();
                    let event = Event::point("gathr.turn.read")
                        .trace(&turn.ids.trace)
                        .chat(&turn.ids.chat)
                        .ok()
                        .attr("gathr.context.ask.chars", chars)
                        .attr("gathr.context.ask.tokens_estimate", chars.div_ceil(4));
                    self.record(event);
                }
                if let Some((message, _)) = read
                    && self.turn.is_none()
                {
                    // Checked live (2.1.318): with no step to stop at, Claude Code ends the
                    // turn, then reads the message sent into it and answers it as a turn of
                    // its own. That turn gets its row here, and the message moves to it.
                    self.begin_read_turn(&message, text.chars().count()).await;
                }
                let turn = self.turn.as_ref().map(|t| t.ids.turn.clone());
                self.publish(&chat, json!({ "type": "read", "turn": turn, "text": text }));
            }
            Frame::ApprovalAsked {
                request_id,
                tool,
                input,
                why,
            } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                let asked = (tool.clone(), input.clone(), why.clone());
                turn.approvals.insert(request_id.clone(), asked);
                let ids = turn.ids.clone();
                self.publish(
                    &ids.chat,
                    json!({ "type": "approval", "turn": ids.turn,
                    "request_id": request_id, "tool": tool, "input": input, "why": why }),
                );
                self.chat_state(&ids).await;
            }
            Frame::Blocked { tool, by, why } => {
                let trace = self.turn.as_ref().map(|t| t.ids.trace.clone());
                let event = Event::point("gathr.approvals.blocked")
                    .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
                    .chat(&chat)
                    .error(&by)
                    .attr("gen_ai.tool.name", tool)
                    .attr("why", why);
                self.record(event);
            }
            Frame::EngineError { kind, message } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.signed_out |= kind == "authentication_failed";
                let trace = turn.ids.trace.clone();
                self.record(
                    Event::point("gathr.engine.error")
                        .trace(&trace)
                        .error(&kind)
                        .attr("message", message),
                );
            }
            Frame::TurnEnd(end) => self.turn_ended(end).await,
            Frame::Background(listed) => self.background(listed).await,
            Frame::Limit {
                status,
                resets_at,
                kind,
            } => {
                if let ("rejected", Some(resets_at)) = (status.as_str(), resets_at) {
                    self.out_of_usage(resets_at, kind).await;
                }
            }
            // A sub-agent's frames stay in the raw record under their Agent call, never in the
            // agent's own messages or tools (friction ledger G24).
            Frame::Fill { used, model } => self.context_filled(used, model).await,
            Frame::Windows(windows) => self.context_window(windows).await,
            Frame::Compacted {
                trigger,
                before,
                after,
            } => self.compacted(trigger, before, after).await,
            // A sub-agent's steps aren't kept, but a field its tool lacks is still recorded (0041).
            Frame::Sub { frame, .. } => {
                if let Frame::ToolShape { tool, field } = *frame {
                    self.tool_shape(tool, field);
                }
            }
            Frame::Thinking { tokens } => self.thinking(tokens).await,
            Frame::Thought => self.thought("done").await,
            // A Codex thought's summary lines come every few seconds: progress, as Claude Code's
            // token counts are, so a long thought is no stall. The lines are never read (0042).
            Frame::Other(method) if method.starts_with("item/reasoning/") => {
                if let Some(turn) = self.turn.as_mut() {
                    turn.progress_at = std::time::Instant::now();
                }
            }
            Frame::ControlAnswer { .. } | Frame::Other(_) => {}
        }
    }

    /// Records a tool call that came without the field its step is read from: the engine's
    /// protocol changed, found from the turn's reference (0019).
    fn tool_shape(&self, tool: String, field: &'static str) {
        let Some(turn) = self.turn.as_ref() else {
            return;
        };
        let event = Event::point("gathr.engine.tool_input")
            .trace(&turn.ids.trace)
            .chat(&turn.ids.chat)
            .error("shape_changed")
            .attr("gen_ai.tool.name", tool)
            .attr("field", field);
        self.record(event);
    }

    /// Records an engine that runs in another permission mode than its agent's approvals: Claude
    /// Code starts in Manual when auto mode isn't offered for the account or model (0029).
    fn check_mode(&self, running: &str) {
        let Some(assigned) = self.assigned.as_ref() else {
            return;
        };
        let expected;
        let given = if assigned.engine == "codex" {
            let (sandbox, policy, _) = gathr::codex::command::policy(assigned.approvals);
            let sandbox = match sandbox {
                "read-only" => "readOnly",
                "workspace-write" => "workspaceWrite",
                _ => "dangerFullAccess",
            };
            expected = format!("{sandbox} {policy}");
            expected.as_str()
        } else {
            command::mode(assigned.approvals)
        };
        if running == given {
            return;
        }
        let trace = self.turn.as_ref().map(|t| t.ids.trace.clone());
        let event = Event::point("gathr.approvals.mode")
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .chat(&assigned.chat)
            .error("other_mode")
            .attr("gathr.approvals", assigned.approvals.as_str())
            .attr("given", given)
            .attr("running", running.to_string());
        self.record(event);
    }

    /// Records a tool the engine offers that's denied, or that nobody has looked at (0001).
    pub(super) fn check_tools(&self, tools: &[String]) {
        let denied: Vec<&String> = tools
            .iter()
            .filter(|t| command::DENIED_TOOLS.iter().any(|(d, _)| d == t))
            .collect();
        let unknown: Vec<&String> = tools
            .iter()
            .filter(|t| !command::KNOWN_TOOLS.contains(&t.as_str()))
            .collect();
        if !denied.is_empty() || !unknown.is_empty() {
            self.record(
                Event::point("gathr.engine.tools")
                    .error("tools_changed")
                    .attr("denied_but_offered", json!(denied))
                    .attr("not_looked_at", json!(unknown)),
            );
        }
    }

    pub(super) async fn tool_started(
        &mut self,
        id: String,
        name: String,
        input: Option<StepInput>,
    ) {
        let Some(turn) = self.turn.as_ref() else {
            return;
        };
        let ids = turn.ids.clone();
        let body = tool_body(&name, "running", None, input.as_ref()).to_string();
        let message = match self.db_message(&ids, "assistant", "tool", &body).await {
            Ok(message) => message,
            Err(why) => return self.save_failed(why, false).await,
        };
        let message_id = message["id"].as_str().unwrap_or_default().to_string();
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            let open = OpenTool {
                message: message_id,
                name,
                started_us: now_us(),
                input,
            };
            turn.tools.insert(id, open);
            turn.tools_started += 1;
        }
    }

    /// What a step keeps of what came back. A file tool whose kind the engine didn't say (Claude
    /// Code, a frame ending several calls) keeps nothing: its text could be the file's own lines,
    /// which a step never keeps (0044, #161's review).
    fn kept_result(&self, id: &str, result: Option<StepResult>, told: bool) -> Option<StepResult> {
        let name = self
            .turn
            .as_ref()?
            .tools
            .get(id)
            .map(|open| open.name.as_str());
        match (told, name) {
            (false, Some("Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit")) => None,
            _ => result,
        }
    }

    /// `said`: what it ran, when the engine says so only as it ends.
    pub(super) async fn tool_ended(
        &mut self,
        id: &str,
        status: &str,
        said: Option<StepInput>,
        result: Option<StepResult>,
    ) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let Some(open) = turn.tools.remove(id) else {
            return;
        };
        let (message_id, name) = (open.message, open.name);
        let input = said.or(open.input);
        turn.push_allowed = false;
        let (ids, span) = (turn.ids.clone(), turn.span.clone());
        let duration_us = now_us() - open.started_us;
        let mut body = tool_body(&name, status, Some(duration_us / 1000), input.as_ref());
        if let Some(result) = result {
            body["result"] = json!(result);
        }
        let (body_text, mid) = (body.to_string(), message_id.clone());
        self.chats
            .writer
            .call(move |conn| store::set_message_body(conn, &mid, &body_text))
            .await;
        let update = json!({ "type": "tool", "message": message_id, "body": body });
        self.publish(&ids.chat, update);
        let mut event = Event::point(&format!("execute_tool {name}"))
            .trace(&ids.trace)
            .chat(&ids.chat)
            .span("end", &span_id())
            .attr("gen_ai.tool.name", name);
        event.parent_span_id = Some(span);
        event.duration_us = Some(duration_us);
        self.record(if status == "ok" {
            event.ok()
        } else {
            event.error("tool_error")
        });
    }

    /// However the turn ends, what the person already saw stays: text streamed so far is
    /// saved, and tool calls and a thought still open show as stopped.
    pub(super) async fn keep_what_was_shown(&mut self) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let text = std::mem::take(&mut turn.partial);
        turn.partial_units = 0;
        let ids = turn.ids.clone();
        let open: Vec<String> = turn.tools.keys().cloned().collect();
        self.save_reply(&ids, text, true).await;
        for id in open {
            self.tool_ended(&id, "stopped", None, None).await;
        }
        if self.turn.as_ref().is_some_and(|t| t.thought.is_some()) {
            self.thought("stopped").await;
        }
    }

    pub(super) async fn turn_ended(&mut self, end: TurnEnd) {
        // A resumed session can answer a notice about a background command before Gathr's
        // message: that result is the notice's, so Gathr's turn runs on (G167).
        if end.notice
            && let Some(turn) = self.turn.as_ref().filter(|t| !t.engine_began)
        {
            let event = Event::point("gathr.turn.notice")
                .trace(&turn.ids.trace)
                .chat(&turn.ids.chat)
                .ok();
            return self.record(event.attr("gathr.turn.id", turn.ids.turn.clone()));
        }
        self.keep_what_was_shown().await;
        let Some(mut turn) = self.turn.take() else {
            return;
        };
        // Ended, so the login needs no guarding: the next start mustn't wait on this one.
        turn.gate.release();
        self.idle_since = std::time::Instant::now();
        // Nothing said and no tool run is no work done: its task isn't taken for done (G167).
        let empty = end.ok && !turn.said && !turn.engine_began && !turn.stopping;
        let status = match () {
            _ if turn.save_failed.is_some() => "failed",
            _ if turn.signed_out => "waiting_sign_in",
            _ if turn.stopping => "stopped",
            _ if empty => "failed",
            _ if end.ok => "done",
            _ => "failed",
        };
        let why = match status {
            "failed" if turn.save_failed.is_some() => turn.save_failed.clone(),
            "failed" if empty => Some(EMPTY.to_string()),
            "failed" => Some(
                end.message
                    .clone()
                    .or(end.reason.clone())
                    .unwrap_or_else(|| end.subtype.clone()),
            ),
            "stopped" => turn.stop_why.clone(),
            _ => None,
        };
        let mut event = Event::point(&format!("invoke_agent {}", self.engine_name()))
            .trace(&turn.ids.trace)
            .chat(&turn.ids.chat)
            .span("end", &turn.span)
            .attr("gathr.turn.status", status)
            .attr("gen_ai.usage.input_tokens", end.usage.input)
            .attr("gen_ai.usage.output_tokens", end.usage.output)
            .attr("gen_ai.usage.cache_read.input_tokens", end.usage.cache_read)
            .attr(
                "gen_ai.usage.cache_creation.input_tokens",
                end.usage.cache_write,
            )
            .attr("gathr.cost_usd", end.cost_usd)
            .attr(
                "gen_ai.response.finish_reasons",
                json!([end.reason.clone().unwrap_or(end.subtype.clone())]),
            );
        event.duration_us = Some(now_us() - turn.sent_us);
        event = match status {
            "failed" => event.error("turn_failed").attr("why", why.clone()),
            "waiting_sign_in" => event.error("signed_out"),
            _ => event.ok(),
        };
        self.record(event);
        if turn.limited && !end.ok && !turn.stopping && !turn.signed_out {
            self.limited_end(&turn.ids, turn.tools_started).await;
            return self.try_next().await;
        }
        if turn.signed_out {
            // The message waits for a sign-in (0002); the engine that failed is closed.
            self.queue.push_front(Queued {
                ids: turn.ids.clone(),
                text: self.text_of(&turn.ids.turn).await,
                held: None,
            });
            self.close("the login was refused").await;
            if let Some(folder) = self.assigned.as_ref().map(|a| a.folder.clone()) {
                let stamp = login_stamp(&Self::credentials(&folder));
                self.refused = Some((folder, stamp, std::time::Instant::now()));
            }
        }
        if status == "failed" {
            self.tell_again(&turn.ids).await;
        }
        self.set_turn(&turn.ids, status, why.as_deref()).await;
        self.try_next().await;
    }

    pub(super) async fn text_of(&self, turn: &str) -> String {
        let turn = turn.to_string();
        self.chats
            .writer
            .call(move |conn| {
                conn.query_row(
                    "SELECT body FROM messages WHERE turn_id = ?1 AND role IN ('user', 'gathr') ORDER BY ts LIMIT 1",
                    [turn],
                    |r| r.get::<_, String>(0),
                )
            })
            .await
            .and_then(Result::ok)
            .unwrap_or_default()
    }
}
