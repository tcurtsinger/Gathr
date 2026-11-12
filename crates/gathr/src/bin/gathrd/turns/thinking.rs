//! That the agent thought, as a time only (0042): a `thinking` message per thought, with no text.
//! Both engines say when a thought starts and ends. Codex may say both at once, when the
//! thought is already over: that mark is timed by the quiet before it.

use std::time::{Duration, Instant};

use gathr::event::Event;
use gathr::ids::now_us;
use gathr::store;
use serde_json::{Value, json};

use super::Task;
use super::driver::Driver;

/// A thought under way: its message, when it began, and the engine's last count of its tokens.
pub(super) struct OpenThought {
    message: String,
    started_us: i64,
    tokens: Option<i64>,
}

/// A thought's start, held: its end may come with it, as the mark of one already over.
pub(super) struct Began {
    at: Instant,
    /// How long the turn had shown no progress when it came.
    quiet: Duration,
}

/// How long a start is held for its end. A mark's two lines were measured 1 to 4 ms apart.
const MARK_WITHIN: Duration = Duration::from_millis(250);

/// A thought timed only by the quiet before its mark is kept from this long: a shorter one is
/// a mark that followed another, not a thought anyone would have seen.
const KEPT_FROM: Duration = Duration::from_secs(1);

fn body(status: &str, duration_ms: Option<i64>, tokens: Option<i64>) -> Value {
    let mut body = json!({ "status": status });
    if let Some(ms) = duration_ms {
        body["duration_ms"] = ms.into();
    }
    if let Some(tokens) = tokens {
        body["tokens"] = tokens.into();
    }
    body
}

impl Task {
    /// The engine is thinking: the first sign of a thought opens its message, later ones bring
    /// its count of tokens. Progress, so a long thought is no stall, but nothing said (G167).
    /// Codex's first sign is held instead, until its end comes with it or `MARK_WITHIN` passes.
    pub(super) async fn thinking(&mut self, tokens: Option<i64>) {
        let held = matches!(self.driver, Driver::Codex(_));
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let quiet = turn.progress_at.elapsed();
        turn.progress_at = Instant::now();
        if let Some(open) = turn.thought.as_mut() {
            open.tokens = tokens.or(open.tokens);
            return;
        }
        // A count while the agent's own tool runs may be a sub-agent's, or one that came after
        // its block: nothing would close a thought opened here until the next words or tool.
        if !turn.tools.is_empty() || turn.thought_began.is_some() {
            return;
        }
        if held {
            let at = Instant::now();
            turn.thought_began = Some(Began { at, quiet });
            return;
        }
        self.open_thought(now_us(), tokens).await;
    }

    /// When the held start, if there is one, opens its thought.
    pub(super) fn thought_due_at(&self) -> Option<tokio::time::Instant> {
        let began = self.turn.as_ref()?.thought_began.as_ref()?;
        Some(tokio::time::Instant::from_std(began.at + MARK_WITHIN))
    }

    /// The held start's end didn't come with it: the thought is under way, since the start. A
    /// line already here is read first, as it may be that end.
    pub(super) async fn thought_due(&mut self) {
        if let Some(Ok(line)) = self.lines.as_mut().map(|lines| lines.try_recv()) {
            return self.on_line(line).await;
        }
        let Some(began) = self.turn.as_mut().and_then(|t| t.thought_began.take()) else {
            return;
        };
        let started_us = now_us() - began.at.elapsed().as_micros() as i64;
        self.open_thought(started_us, None).await;
    }

    async fn open_thought(&mut self, started_us: i64, tokens: Option<i64>) {
        let Some(ids) = self.turn.as_ref().map(|t| t.ids.clone()) else {
            return;
        };
        let running = body("running", None, None).to_string();
        let kind = ("assistant", "thinking");
        let message = match self
            .db_message_from(&ids, kind, &running, Some(started_us))
            .await
        {
            Ok(message) => message,
            Err(why) => return self.save_failed(why, false).await,
        };
        let open = OpenThought {
            message: message["id"].as_str().unwrap_or_default().to_string(),
            started_us,
            tokens,
        };
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            turn.thought = Some(open);
        }
    }

    /// A thought ended as `status`. One under way closes with the time it ran. With none under
    /// way this is the mark of one already over (Codex, its start still held): it lasted the
    /// quiet before it, and is kept when no step was running and that was long enough.
    pub(super) async fn thought(&mut self, status: &str) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let before = turn.thought_began.take().map(|began| began.quiet);
        let quiet = turn.progress_at.elapsed() + before.unwrap_or_default();
        turn.progress_at = Instant::now();
        let ids = turn.ids.clone();
        let (duration_us, tokens) = match turn.thought.take() {
            Some(open) => {
                let duration_us = now_us() - open.started_us;
                let body = body(status, Some(duration_us / 1000), open.tokens);
                let (text, id) = (body.to_string(), open.message.clone());
                let set = move |conn: &mut rusqlite::Connection| {
                    store::set_message_body(conn, &id, &text)
                };
                self.chats.writer.call(set).await;
                let update = json!({ "type": "tool", "message": open.message, "body": body });
                self.publish(&ids.chat, update);
                (duration_us, open.tokens)
            }
            None if turn.tools.is_empty() && quiet >= KEPT_FROM => {
                // Its time is when the quiet began, but never before the turn's last message:
                // moved later, it is that much shorter, so it never ends after its mark.
                let ended = now_us();
                let quiet_began = ended - quiet.as_micros() as i64;
                let (chat, id) = (ids.chat.clone(), ids.turn.clone());
                let after = move |conn: &mut rusqlite::Connection| {
                    store::after_last(conn, &chat, &id, quiet_began)
                };
                let began = self.chats.writer.call(after).await.and_then(Result::ok);
                let began = began.unwrap_or(quiet_began);
                let duration_us = ended - began;
                if duration_us < KEPT_FROM.as_micros() as i64 {
                    return;
                }
                let body = body(status, Some(duration_us / 1000), None).to_string();
                let kind = ("assistant", "thinking");
                match self.db_message_from(&ids, kind, &body, Some(began)).await {
                    Ok(message) => {
                        self.publish(&ids.chat, json!({ "type": "message", "message": message }))
                    }
                    Err(why) => return self.save_failed(why, false).await,
                }
                (duration_us, None)
            }
            None => return,
        };
        let mut event = Event::point("gathr.turn.thinking")
            .trace(&ids.trace)
            .chat(&ids.chat);
        if let Some(tokens) = tokens {
            event = event.attr("gathr.thinking.tokens_estimate", tokens);
        }
        event.duration_us = Some(duration_us);
        self.record(match status {
            "done" => event.ok(),
            _ => event.error(status),
        });
    }
}
