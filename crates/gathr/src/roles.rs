//! The few lines a team's briefing gives each role, only where Gathr's tools and state don't already
//! make the agent do it, each shipped only after the trial on real engines kept it (0066).

use crate::team::Role;

/// L1, the lead's.
pub const LEAD: &str = "Before you ask the person, look in the code and docs: ask only what they don't answer, \
                        with choices and your pick first.";
/// L2, a member's.
pub const MEMBER: &str = "Make the smallest change that does what was asked: no fallback, guard, abstraction or \
                          compatibility nobody asked for, and remove what it replaces.";
/// L3, everyone's in a team.
pub const AGREE: &str = "When you only agree with another agent's message, give it a thumbs-up with react instead of replying.";
/// L4, a code review's, before its unasked line (`review_brief` 2).
pub const REVIEW: &str = "Say what is wrong or missing against the ask, as wrong notes.";

/// Each shipped line by its id in the trial's file, `crates/gathr/trial/results.json`.
pub const SHIPPED: [(&str, &str); 4] =
    [("L1", LEAD), ("L2", MEMBER), ("L3", AGREE), ("L4", REVIEW)];

/// A role's lines in a team's briefing: none in the trial's without arm.
pub fn lines(role: Role) -> Vec<&'static str> {
    if without() {
        return Vec::new();
    }
    let own = match role {
        Role::Supervisor => Some(LEAD),
        Role::Member => Some(MEMBER),
        Role::Reviewer => None,
    };
    own.into_iter().chain([AGREE]).collect()
}

/// The trial's without arm: a debug build's service started with `GATHR_TRIAL_WITHOUT` gives no
/// role lines and the review brief's version 1. A release build never reads it.
pub fn without() -> bool {
    cfg!(debug_assertions) && std::env::var_os("GATHR_TRIAL_WITHOUT").is_some()
}
