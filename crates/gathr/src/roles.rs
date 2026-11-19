//! The few lines a team's briefing gives each role, only where Gathr's tools and state don't already
//! make the agent do it. L2 and L3 shipped after the trial on real engines kept them (0066); the
//! lines of the open check (0080) ship untried, by the owner's call of 2026-11-19.

use crate::team::Role;

/// L2, a member's.
pub const MEMBER: &str = "Make the smallest change that does what was asked: no fallback, guard, abstraction or \
                          compatibility nobody asked for, and remove what it replaces.";
/// L3, everyone's in a team.
pub const AGREE: &str = "When you only agree with another agent's message, give it a thumbs-up with react instead of replying.";
/// L5, everyone's in a team.
pub const QUOTE: &str = "When you answer a point, quote its line first as \"> Name: their words\", then reply under \
                         it. Speak only as yourself, in a few lines: this is a conversation, not a report.";
/// L6, a member's and a reviewer's.
pub const ARGUE: &str =
    "If you disagree with the lead's call, argue your case once, in words, then follow it.";
/// L7, the lead's.
pub const LEAD: &str = "After you answer, the members who took no part check it, and when one questions it you have \
                        the last word. When members disagree, make the call and give the reason. Don't pass on what \
                        members said.";

/// Each line the trial kept, by its id in the trial's file, `crates/gathr/trial/results.json`.
pub const SHIPPED: [(&str, &str); 2] = [("L2", MEMBER), ("L3", AGREE)];

/// Each line shipped without a trial (0080): the room's next run is their trial.
pub const UNTRIED: [(&str, &str); 3] = [("L5", QUOTE), ("L6", ARGUE), ("L7", LEAD)];

/// A role's lines in a team's briefing: none in the trial's without arm.
pub fn lines(role: Role) -> Vec<&'static str> {
    match (without(), role) {
        (true, _) => Vec::new(),
        (false, Role::Member) => vec![MEMBER, AGREE, QUOTE, ARGUE],
        (false, Role::Reviewer) => vec![AGREE, QUOTE, ARGUE],
        (false, Role::Supervisor) => vec![AGREE, QUOTE, LEAD],
    }
}

/// The trial's without arm: a debug build's service started with `GATHR_TRIAL_WITHOUT` gives no
/// role lines. A release build never reads it.
pub fn without() -> bool {
    cfg!(debug_assertions) && std::env::var_os("GATHR_TRIAL_WITHOUT").is_some()
}
