//! One row of the event record (docs/decisions/0011-event-record.md).

use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::ids::{now_us, ulid};
use crate::redact::redact_json;

#[derive(Debug, Clone)]
pub struct Event {
    pub ts: i64,
    pub name: String,
    pub phase: &'static str,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<i64>,
    pub duration_us: Option<i64>,
    pub status: Option<&'static str>,
    pub error_type: Option<String>,
    pub attrs: Map<String, Value>,
}

impl Event {
    /// An event with no duration, now.
    pub fn point(name: &str) -> Self {
        Event {
            ts: now_us(),
            name: name.to_string(),
            phase: "point",
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            conversation_id: None,
            task_id: None,
            duration_us: None,
            status: None,
            error_type: None,
            attrs: Map::new(),
        }
    }

    pub fn trace(mut self, trace_id: &str) -> Self {
        self.trace_id = Some(trace_id.to_string());
        self
    }

    pub fn chat(mut self, chat: &str) -> Self {
        self.conversation_id = Some(chat.to_string());
        self
    }

    /// The task it's about (0010).
    pub fn task(mut self, number: i64) -> Self {
        self.task_id = Some(number);
        self
    }

    /// The start or end of a span: `phase` is `start` or `end`.
    pub fn span(mut self, phase: &'static str, span_id: &str) -> Self {
        self.phase = phase;
        self.span_id = Some(span_id.to_string());
        self
    }

    pub fn ok(mut self) -> Self {
        self.status = Some("ok");
        self
    }

    /// Marks the event failed, with OpenTelemetry's `error.type`: a short class of failure.
    pub fn error(mut self, error_type: &str) -> Self {
        self.status = Some("error");
        self.error_type = Some(error_type.to_string());
        self
    }

    pub fn attr(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.attrs.insert(key.to_string(), value.into());
        self
    }

    /// Writes the row. Every string in `attrs` is redacted first.
    pub fn insert(&self, conn: &Connection) -> rusqlite::Result<()> {
        let attrs = (!self.attrs.is_empty())
            .then(|| redact_json(&Value::Object(self.attrs.clone())).to_string());
        conn.prepare_cached(
            "INSERT INTO events (id, ts, name, phase, trace_id, span_id, parent_span_id, conversation_id,
                duration_us, status, error_type, attrs, task_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, jsonb(?12), ?13)",
        )?
        .execute(params![
            ulid(self.ts),
            self.ts,
            self.name,
            self.phase,
            self.trace_id,
            self.span_id,
            self.parent_span_id,
            self.conversation_id,
            self.duration_us,
            self.status,
            self.error_type,
            attrs,
            self.task_id.map(|n| n.to_string()),
        ])?;
        Ok(())
    }
}
