//! Reading the engine's output: each line recorded, then read as frames that move the turn on.

use gathr::claude::command;
use gathr::claude::engine::Line;
use gathr::claude::frames::{self, Frame, TurnEnd};
use gathr::event::Event;
use gathr::ids::{now_us, span_id};
use gathr::store;
use serde_json::{Value, json};

use super::{Queued, Task, login_stamp};
use gathr::claude::login;

impl Task {
    pub(super) async fn on_line(&mut self, line: Line) {
        let pid = self.engine.as_ref().map_or(0, |e| e.pid);
        let turn = self.turn.as_ref().map(|t| t.turn.clone());
        let (direction, text) = match &line {
            Line::Out(text) => ("out", text),
            Line::Err(text) => ("err", text),
        };
        // Written, redacted, before it's acted on (0011).
        self.raw(turn.as_deref(), pid, direction, text).await;
        let Line::Out(text) = line else {
            self.record(
                Event::point("gathr.engine.stderr")
                    .chat(&self.chat)
                    .attr("line", text.clone()),
            );
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
                    .chat(&self.chat)
                    .error("malformed")
                    .attr("why", e.to_string()),
            ),
        }
    }

    pub(super) async fn on_frame(&mut self, frame: Frame) {
        let chat = self.chat.clone();
        if matches!(
            frame,
            Frame::Replied | Frame::Text(_) | Frame::ToolStart { .. }
        ) && let Some(turn) = self.turn.as_mut()
        {
            turn.gate.release();
        }
        match frame {
            Frame::Replied => {}
            Frame::Init {
                session_id, tools, ..
            } => {
                self.check_tools(&tools);
                let saved = self
                    .chats
                    .writer
                    .call(move |conn| store::set_engine_session(conn, &chat, &session_id))
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
                turn.partial.push_str(&text);
                let first = !std::mem::replace(&mut turn.streamed, true);
                let (id, trace, sent) = (turn.turn.clone(), turn.trace.clone(), turn.sent_us);
                self.publish(json!({ "type": "delta", "chat": chat, "turn": id, "text": text }));
                if first {
                    let mut event = Event::point("gathr.turn.first_token").trace(&trace).ok();
                    event.duration_us = Some(now_us() - sent);
                    self.record(event);
                }
            }
            Frame::Text(text) => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.partial.clear();
                let id = turn.turn.clone();
                if !text.is_empty() {
                    let message = self.db_message(&id, "assistant", "text", &text).await;
                    self.publish(json!({ "type": "message", "message": message }));
                }
            }
            Frame::ToolStart { id, name } => self.tool_started(id, name).await,
            Frame::ToolEnd { id, ok } => {
                self.tool_ended(&id, if ok { "ok" } else { "error" }).await
            }
            Frame::Read(text) => {
                let unread = self.unread.iter().position(|(_, t)| *t == text);
                if let Some((message, _)) = unread.map(|i| self.unread.remove(i))
                    && self.turn.is_none()
                {
                    // Checked live (2.1.318): with no step to stop at, Claude Code ends the
                    // turn, then reads the message sent into it and answers it as a turn of
                    // its own. That turn gets its row here, and the message moves to it.
                    self.begin_read_turn(&message).await;
                }
                let turn = self.turn.as_ref().map(|t| t.turn.clone());
                self.publish(json!({ "type": "read", "chat": chat, "turn": turn, "text": text }));
            }
            Frame::ApprovalAsked {
                request_id,
                tool,
                input,
            } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.approvals.insert(request_id.clone(), input.clone());
                let id = turn.turn.clone();
                self.publish(json!({ "type": "approval", "chat": chat, "turn": id,
                    "request_id": request_id, "tool": tool, "input": input }));
            }
            Frame::EngineError { kind, message } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.signed_out |= kind == "authentication_failed";
                let trace = turn.trace.clone();
                self.record(
                    Event::point("gathr.engine.error")
                        .trace(&trace)
                        .error(&kind)
                        .attr("message", message),
                );
            }
            Frame::TurnEnd(end) => self.turn_ended(end).await,
            // A sub-agent's frames stay in the raw record under their Agent call, never in the
            // agent's own messages or tools (friction ledger G24).
            Frame::Sub { .. } | Frame::ControlAnswer { .. } | Frame::Other(_) => {}
        }
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
                    .chat(&self.chat)
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
        let turn_id = turn.turn.clone();
        let body = json!({ "tool": name, "status": "running" }).to_string();
        let message = self.db_message(&turn_id, "assistant", "tool", &body).await;
        let message_id = message["id"].as_str().unwrap_or_default().to_string();
        self.publish(json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            turn.tools.insert(id, (message_id, name, now_us()));
        }
    }

    pub(super) async fn tool_ended(&mut self, id: &str, status: &str) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let Some((message_id, name, started)) = turn.tools.remove(id) else {
            return;
        };
        let (trace, span) = (turn.trace.clone(), turn.span.clone());
        let duration_us = now_us() - started;
        let body = json!({ "tool": name, "status": status, "duration_ms": duration_us / 1000 });
        let (body_text, mid) = (body.to_string(), message_id.clone());
        self.chats
            .writer
            .call(move |conn| store::set_message_body(conn, &mid, &body_text))
            .await;
        self.publish(
            json!({ "type": "tool", "chat": self.chat, "message": message_id, "body": body }),
        );
        let mut event = Event::point(&format!("execute_tool {name}"))
            .trace(&trace)
            .chat(&self.chat)
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
        let id = turn.turn.clone();
        let open: Vec<String> = turn.tools.keys().cloned().collect();
        if !text.is_empty() {
            let message = self.db_message(&id, "assistant", "text", &text).await;
            self.publish(json!({ "type": "message", "message": message }));
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
        let status = match () {
            _ if turn.signed_out => "waiting_sign_in",
            _ if turn.stopping => "stopped",
            _ if end.ok => "done",
            _ => "failed",
        };
        let why = (status == "failed").then(|| {
            end.message
                .clone()
                .or(end.reason.clone())
                .unwrap_or_else(|| end.subtype.clone())
        });
        let mut event = Event::point("invoke_agent claude")
            .trace(&turn.trace)
            .chat(&self.chat)
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
        if turn.signed_out {
            // The message waits for a sign-in (0002); the engine that failed is closed.
            self.queue.push_front(Queued {
                turn: turn.turn.clone(),
                trace: turn.trace.clone(),
                text: self.text_of(&turn.turn).await,
            });
            self.close("the login was refused").await;
            let file = login::credentials(&self.chats.account_folder());
            self.refused = Some((login_stamp(&file), std::time::Instant::now()));
        }
        self.set_turn(&turn.turn, &turn.trace, status, why.as_deref())
            .await;
        self.try_next().await;
    }

    pub(super) async fn text_of(&self, turn: &str) -> String {
        let turn = turn.to_string();
        self.chats
            .writer
            .call(move |conn| {
                conn.query_row(
                    "SELECT body FROM messages WHERE turn_id = ?1 AND role = 'user' ORDER BY ts LIMIT 1",
                    [turn],
                    |r| r.get::<_, String>(0),
                )
            })
            .await
            .and_then(Result::ok)
            .unwrap_or_default()
    }
}
