//! Which of Gathr's words opened a turn, and their version (Stage 3, part A). Each turn records
//! its kind and, for a kind with fixed text, that text's version. The words stay where they are
//! built.

mod counted;

pub use counted::{Counted, counted};

pub const CARRY_ON: i64 = 2;
pub const CHECK: i64 = 3;
pub const LAST_WORD: i64 = 2;
pub const REPLY: i64 = 2;
pub const PICKED: i64 = 1;
pub const SCHEDULED: i64 = 1;
pub const BRIEF: i64 = 1;

/// What opened a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// An update, Stop now or a usage limit ended the turn.
    CarryOn,
    /// A member checks the lead's answer (0080).
    Check,
    /// The lead's last word on what members questioned (0080).
    LastWord,
    /// The person replied under a message (0076): Gathr's line names what the reply answers, and who
    /// else got it.
    Reply,
    /// The person picked several agents (0028): Gathr's line names the others. Or the person answered a
    /// question: the line carries the answer (`questions`).
    Picked,
    /// What the person had an agent schedule (`schedules`): Gathr's line says which schedule and when it was due,
    /// then the words, drawn as the person's.
    Scheduled,
    /// The brief the person's assistant wrote for a project it proposed, sent as the person made it (`proposals`):
    /// Gathr's line says who wrote it, then the words, drawn as the person's.
    Brief,
    Person,
    Teammate,
    /// Compact, from the agent panel: the engine's own command, not words to a model.
    Compaction,
    /// A turn the engine began by itself when a background command ended.
    Engine,
}

impl Opened {
    pub fn as_str(self) -> &'static str {
        match self {
            Opened::CarryOn => "carry_on",
            Opened::Check => "check",
            Opened::LastWord => "last_word",
            Opened::Reply => "reply",
            Opened::Picked => "picked",
            Opened::Scheduled => "scheduled",
            Opened::Brief => "brief",
            Opened::Person => "person",
            Opened::Teammate => "teammate",
            Opened::Compaction => "compaction",
            Opened::Engine => "engine",
        }
    }

    /// The version of the kind's fixed text; none for a kind whose text isn't Gathr's.
    pub fn version(self) -> Option<i64> {
        match self {
            Opened::CarryOn => Some(CARRY_ON),
            Opened::Check => Some(CHECK),
            Opened::LastWord => Some(LAST_WORD),
            Opened::Reply => Some(REPLY),
            Opened::Picked => Some(PICKED),
            Opened::Scheduled => Some(SCHEDULED),
            Opened::Brief => Some(BRIEF),
            Opened::Person | Opened::Teammate | Opened::Compaction | Opened::Engine => None,
        }
    }
}
