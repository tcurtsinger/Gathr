//! The two protocols at the turn boundary. Process ownership stays shared.
use gathr::claude::frames::{self, Frame};
use gathr::codex::rpc::{Opening, Read, Session};
use serde_json::Value;

#[derive(Default)]
pub enum Driver {
    #[default]
    Claude,
    Codex(Box<Session>),
}

impl Driver {
    pub fn codex(opening: Opening) -> (Self, Value) {
        let (session, first) = Session::open(opening);
        (Self::Codex(Box::new(session)), first)
    }
    pub fn read(&mut self, value: &Value) -> Read {
        match self {
            Self::Claude => Read {
                frames: frames::read(value),
                send: Vec::new(),
            },
            Self::Codex(session) => session.read(value),
        }
    }
    pub fn user(&mut self, text: &str) -> Option<Value> {
        match self {
            Self::Claude => Some(frames::user(text)),
            Self::Codex(session) => session.user(text),
        }
    }
    pub fn interrupt(&mut self, id: &str) -> Option<Value> {
        match self {
            Self::Claude => Some(frames::interrupt(id)),
            Self::Codex(session) => session.interrupt(),
        }
    }
    pub fn approval(&mut self, id: &str, allow: bool, input: &Value) -> Option<Value> {
        match self {
            Self::Claude => Some(frames::approval(id, allow, input)),
            Self::Codex(session) => session.approval(id, allow),
        }
    }
    pub fn needs_recovery(frames: &[Frame]) -> bool {
        frames.iter().any(|f| matches!(f, Frame::EngineError { kind, .. } if kind == "initialize" || kind == "thread" || kind == "engine_version" || kind == "models" || kind == "turn" || kind == "steer"))
    }
}
