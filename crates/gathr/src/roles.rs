//! The few lines a team's briefing gives each role, only where Gathr's tools and state don't already
//! make the agent do it, each shipped only after the trial on real engines kept it (0066).

use crate::team::Role;

/// L2, a member's.
pub const MEMBER: &str = "Make the smallest change that does what was asked: no fallback, guard, abstraction or \
                          compatibility nobody asked for, and remove what it replaces.";
/// L3, everyone's in a team.
pub const AGREE: &str = "When you only agree with another agent's message, give it a thumbs-up with react instead of replying.";

/// Each shipped line by its id in the trial's file, `crates/gathr/trial/results.json`.
pub const SHIPPED: [(&str, &str); 2] = [("L2", MEMBER), ("L3", AGREE)];

/// A role's lines in a team's briefing: none in the trial's without arm.
pub fn lines(role: Role) -> Vec<&'static str> {
    match (without(), role) {
        (true, _) => Vec::new(),
        (false, Role::Member) => vec![MEMBER, AGREE],
        (false, _) => vec![AGREE],
    }
}

/// The trial's without arm: a debug build's service started with `GATHR_TRIAL_WITHOUT` gives no
/// role lines. A release build never reads it.
pub fn without() -> bool {
    cfg!(debug_assertions) && std::env::var_os("GATHR_TRIAL_WITHOUT").is_some()
}
