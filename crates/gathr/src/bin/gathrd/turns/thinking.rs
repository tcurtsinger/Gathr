//! That the agent thought, as a time only (0042): a `thinking` message per thought, with no text.
//! Claude Code says when a thought starts and ends; Codex only that one is over.

use std::time::{Duration, Instant};

use gathr::event::Event;
use gathr::ids::now_us;
use gathr::store;
use serde_json::{Value, json};

use super::Task;

/// A thought under way: its message, when it began, and the engine's last count of its tokens.
pub(super) struct OpenThought {
    message: String,
    started_us: i64,
    tokens: Option<i64>,
}

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
    pub(super) async fn thinking(&mut self, tokens: Option<i64>) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        turn.progress_at = Instant::now();
        if let Some(open) = turn.thought.as_mut() {
            open.tokens = tokens.or(open.tokens);
            return;
        }
        let ids = turn.ids.clone();
        let running = body("running", None, None).to_string();
        let message = match self
            .db_message(&ids, "assistant", "thinking", &running)
            .await
        {
            Ok(message) => message,
            Err(why) => return self.save_failed(why, false).await,
        };
        let open = OpenThought {
            message: message["id"].as_str().unwrap_or_default().to_string(),
            started_us: now_us(),
            tokens,
        };
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            turn.thought = Some(open);
        }
    }

    /// A thought ended as `status`. One under way closes with the time it ran. With none under
    /// way this is the engine's only sign of one (Codex): it lasted the quiet since the turn last
    /// showed progress, and is kept when no step was running and that was long enough.
    pub(super) async fn thought(&mut self, status: &str) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let quiet = turn.progress_at.elapsed();
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
                let duration_us = quiet.as_micros() as i64;
                let body = body(status, Some(duration_us / 1000), None).to_string();
                let (kind, began) = (("assistant", "thinking"), Some(now_us() - duration_us));
                match self.db_message_from(&ids, kind, &body, began).await {
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
