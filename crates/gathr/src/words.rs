//! Which of Gathr's words opened a turn, and their version (Stage 3, part A). Each turn records
//! its kind and, for a kind with fixed text, that text's version. The words stay where they are
//! built; kept samples of each kind's text in `tests/words/samples.rs` hold its version, so a
//! change of words without a new version fails a test.

mod counted;

pub use counted::{Counted, counted};

pub const BRIEF: i64 = 3;
pub const REVIEW_BRIEF: i64 = 1;
pub const REQUEST_BRIEF: i64 = 2;
pub const REPORT_ASK: i64 = 1;
pub const HEAR_BACK: i64 = 3;
pub const ANSWERED: i64 = 2;
pub const WAKE: i64 = 1;
pub const SILENT: i64 = 1;
pub const WAIT_FIRED: i64 = 1;
pub const STALE: i64 = 1;
pub const LANDING: i64 = 1;
pub const CARRY_ON: i64 = 2;
pub const CHECK: i64 = 1;
pub const LAST_WORD: i64 = 1;
pub const SENT_BACK: i64 = 1;
pub const RESIZED: i64 = 1;
pub const LIKED: i64 = 1;
pub const REPLY: i64 = 2;
pub const PICKED: i64 = 1;
/// A team's role lines in the briefing (`crate::roles`, 0066): no turn opens with them.
pub const ROLE: i64 = 2;

/// What opened a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// A work task's brief, first or again with "Continue from here".
    Brief,
    ReviewBrief,
    /// The lead's brief on a member's request for the person.
    RequestBrief,
    /// The one ask for a report.
    ReportAsk,
    /// What became of the tasks the agent gave.
    HearBack,
    /// The person answered tasks the agent gave.
    Answered,
    /// A chat where nothing moves wakes its lead.
    Wake,
    /// A member's silent turn is told to its lead.
    Silent,
    WaitFired,
    /// The lead is told of stale tasks.
    Stale,
    /// A PR Gathr can't land is handed to its reviewer.
    Landing,
    /// An update, Stop now or a usage limit ended the turn.
    CarryOn,
    /// A member checks the lead's answer (0080).
    Check,
    /// The lead's last word on what members questioned (0080).
    LastWord,
    /// The person sent a plan back to its lead.
    SentBack,
    /// The person changed an ask's size (0063).
    Resized,
    /// The person liked the agent's message (0057).
    Liked,
    /// The person replied in a thread (0076): Gathr's line names what the reply answers, and who else got it.
    Reply,
    /// The person picked several agents (0028): Gathr's line names the others.
    Picked,
    Person,
    Teammate,
    /// Compact, from the agent panel: the engine's own command, not words to a model.
    Compaction,
    /// A turn the engine began by itself when a background command ended.
    Engine,
}

impl Opened {
    pub const ALL: [Opened; 23] = [
        Opened::Brief,
        Opened::ReviewBrief,
        Opened::RequestBrief,
        Opened::ReportAsk,
        Opened::HearBack,
        Opened::Answered,
        Opened::Wake,
        Opened::Silent,
        Opened::WaitFired,
        Opened::Stale,
        Opened::Landing,
        Opened::CarryOn,
        Opened::Check,
        Opened::LastWord,
        Opened::SentBack,
        Opened::Resized,
        Opened::Liked,
        Opened::Reply,
        Opened::Picked,
        Opened::Person,
        Opened::Teammate,
        Opened::Compaction,
        Opened::Engine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Opened::Brief => "brief",
            Opened::ReviewBrief => "review_brief",
            Opened::RequestBrief => "request_brief",
            Opened::ReportAsk => "report_ask",
            Opened::HearBack => "hear_back",
            Opened::Answered => "answered",
            Opened::Wake => "wake",
            Opened::Silent => "silent",
            Opened::WaitFired => "wait_fired",
            Opened::Stale => "stale",
            Opened::Landing => "landing",
            Opened::CarryOn => "carry_on",
            Opened::Check => "check",
            Opened::LastWord => "last_word",
            Opened::SentBack => "sent_back",
            Opened::Resized => "resized",
            Opened::Liked => "liked",
            Opened::Reply => "reply",
            Opened::Picked => "picked",
            Opened::Person => "person",
            Opened::Teammate => "teammate",
            Opened::Compaction => "compaction",
            Opened::Engine => "engine",
        }
    }

    /// The version of the kind's fixed text; none for a kind whose text isn't Gathr's.
    pub fn version(self) -> Option<i64> {
        match self {
            Opened::Brief => Some(BRIEF),
            Opened::ReviewBrief => Some(REVIEW_BRIEF),
            Opened::RequestBrief => Some(REQUEST_BRIEF),
            Opened::ReportAsk => Some(REPORT_ASK),
            Opened::HearBack => Some(HEAR_BACK),
            Opened::Answered => Some(ANSWERED),
            Opened::Wake => Some(WAKE),
            Opened::Silent => Some(SILENT),
            Opened::WaitFired => Some(WAIT_FIRED),
            Opened::Stale => Some(STALE),
            Opened::Landing => Some(LANDING),
            Opened::CarryOn => Some(CARRY_ON),
            Opened::Check => Some(CHECK),
            Opened::LastWord => Some(LAST_WORD),
            Opened::SentBack => Some(SENT_BACK),
            Opened::Resized => Some(RESIZED),
            Opened::Liked => Some(LIKED),
            Opened::Reply => Some(REPLY),
            Opened::Picked => Some(PICKED),
            Opened::Person | Opened::Teammate | Opened::Compaction | Opened::Engine => None,
        }
    }
}
