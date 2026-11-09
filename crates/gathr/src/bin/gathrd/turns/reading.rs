//! Reading the engine's output: each line recorded, then read as frames that move the turn on.

use gathr::claude::command;
use gathr::claude::engine::Line;
use gathr::claude::frames::{self, Frame, TurnEnd};
use gathr::claude::login;
use gathr::event::Event;
use gathr::ids::{now_us, span_id};
use gathr::store;
use serde_json::{Value, json};

use super::{Queued, Task, login_stamp};

impl Task {
    pub(super) async fn on_line(&mut self, line: Line) {
        let pid = self.engine.as_ref().map_or(0, |e| e.pid);
        let turn = self.turn.as_ref().map(|t| t.ids.turn.clone());
        let (direction, text) = match &line {
            Line::Out(text) => ("out", text),
            Line::Err(text) => ("err", text),
        };
        // Written, redacted, before it's acted on (0011).
        self.raw(turn.as_deref(), pid, direction, text).await;
        let Line::Out(text) = line else {
            self.record(Event::point("gathr.engine.stderr").attr("line", text.clone()));
            if !text.trim().is_empty() {
                self.last_stderr = Some(text.clone());
            }
            return;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                for frame in frames::read(&value) {
                    self.on_frame(frame).await;
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
            Frame::TextDelta(_) | Frame::Text(_) | Frame::ToolStart { .. } | Frame::ToolEnd { .. }
        ) && let Some(turn) = self.turn.as_mut()
        {
            turn.progress_at = std::time::Instant::now();
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
                if !text.is_empty() {
                    let message = self.db_message(&ids, "assistant", "text", &text).await;
                    self.publish(&ids.chat, json!({ "type": "message", "message": message }));
                }
            }
            Frame::ToolStart { id, name } => self.tool_started(id, name).await,
            Frame::ToolEnd { id, ok } => {
                self.tool_ended(&id, if ok { "ok" } else { "error" }).await
            }
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
            Frame::Sub { .. } | Frame::ControlAnswer { .. } | Frame::Other(_) => {}
        }
    }

    /// Records an engine that runs in another permission mode than its agent's approvals: Claude
    /// Code starts in Manual when auto mode isn't offered for the account or model (0029).
    fn check_mode(&self, running: &str) {
        let Some(assigned) = self.assigned.as_ref() else {
            return;
        };
        let given = command::mode(assigned.approvals);
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

    pub(super) async fn tool_started(&mut self, id: String, name: String) {
        let Some(turn) = self.turn.as_ref() else {
            return;
        };
        let ids = turn.ids.clone();
        let body = json!({ "tool": name, "status": "running" }).to_string();
        let message = self.db_message(&ids, "assistant", "tool", &body).await;
        let message_id = message["id"].as_str().unwrap_or_default().to_string();
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            turn.tools.insert(id, (message_id, name, now_us()));
            turn.tools_started += 1;
        }
    }

    pub(super) async fn tool_ended(&mut self, id: &str, status: &str) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let Some((message_id, name, started)) = turn.tools.remove(id) else {
            return;
        };
        turn.push_allowed = false;
        let (ids, span) = (turn.ids.clone(), turn.span.clone());
        let duration_us = now_us() - started;
        let body = json!({ "tool": name, "status": status, "duration_ms": duration_us / 1000 });
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
    /// saved, and tool calls still open show as stopped.
    pub(super) async fn keep_what_was_shown(&mut self) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let text = std::mem::take(&mut turn.partial);
        turn.partial_units = 0;
        let ids = turn.ids.clone();
        let open: Vec<String> = turn.tools.keys().cloned().collect();
        if !text.is_empty() {
            let message = self.db_message(&ids, "assistant", "text", &text).await;
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        }
        for id in open {
            self.tool_ended(&id, "stopped").await;
        }
    }

    pub(super) async fn turn_ended(&mut self, end: TurnEnd) {
        self.keep_what_was_shown().await;
        let Some(mut turn) = self.turn.take() else {
            return;
        };
        // Ended, so the login needs no guarding: the next start mustn't wait on this one.
        turn.gate.release();
        self.idle_since = std::time::Instant::now();
        let status = match () {
            _ if turn.signed_out => "waiting_sign_in",
            _ if turn.stopping => "stopped",
            _ if end.ok => "done",
            _ => "failed",
        };
        let why = match status {
            "failed" => Some(
                end.message
                    .clone()
                    .or(end.reason.clone())
                    .unwrap_or_else(|| end.subtype.clone()),
            ),
            "stopped" => turn.stop_why.clone(),
            _ => None,
        };
        let mut event = Event::point("invoke_agent claude")
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
                let stamp = login_stamp(&login::credentials(&folder));
                self.refused = Some((folder, stamp, std::time::Instant::now()));
            }
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
