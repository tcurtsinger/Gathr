//! How a turn that stopped ended, told apart for the person (0051): their own stop, a restart that cut it, or Gathr
//! ending it for a reason of its own. Read from the turn's status and `why`, which Gathr writes in the words here.

/// Why a turn that was running when the service stopped reads stopped: a restart cut it.
pub const CUT_BY_RESTART: &str = "the service stopped during this turn";

/// Why a turn the usage limit ended after a tool ran stopped: its carry-on waits for the account.
pub const OUT_OF_USAGE: &str = "the account is out of usage";

const WAITS_FOR_PLAN: &str = "waits for its plan";

/// Why a turn of a task that was cancelled stops.
pub fn cancelled(task: i64, why: &str) -> String {
    format!("its task #{task} was cancelled: {why}")
}

/// Why a queued turn of a task that now waits for its ask's plan doesn't start (0063).
pub fn waits_for_plan(task: i64) -> String {
    format!("its task #{task} {WAITS_FOR_PLAN}")
}

/// One word for who or what stopped a turn: `you` (the person's stop, which says no why), `restart`, or Gathr's
/// own: `plan`, `cancelled`, `usage`, or `gathr` for anything else. None for a turn that didn't stop.
pub fn word(status: &str, why: Option<&str>) -> Option<&'static str> {
    if status != "stopped" {
        return None;
    }
    let Some(why) = why else {
        return Some("you");
    };
    Some(match why {
        CUT_BY_RESTART => "restart",
        OUT_OF_USAGE => "usage",
        _ if why.starts_with("its task #") && why.contains(" was cancelled: ") => "cancelled",
        _ if why.starts_with("its task #") && why.ends_with(WAITS_FOR_PLAN) => "plan",
        _ => "gathr",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stop_reads_as_who_or_what_made_it() {
        let read = |why: Option<&str>| word("stopped", why);
        assert_eq!(read(None), Some("you"));
        assert_eq!(read(Some(CUT_BY_RESTART)), Some("restart"));
        assert_eq!(read(Some(OUT_OF_USAGE)), Some("usage"));
        assert_eq!(read(Some(&waits_for_plan(4))), Some("plan"));
        let approved = cancelled(12, "version 1 was approved");
        assert_eq!(read(Some(&approved)), Some("cancelled"));
        // A reason that only names a plan is the cancel's, not a hold for one.
        assert_eq!(
            read(Some(&cancelled(7, "waits for its plan"))),
            Some("cancelled")
        );
        assert_eq!(read(Some("its chat is archived")), Some("gathr"));
        assert_eq!(word("failed", Some(CUT_BY_RESTART)), None);
        assert_eq!(word("done", None), None);
    }
}
