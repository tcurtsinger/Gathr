//! How a turn that stopped ended, told apart for the person (0051): their own stop, a restart that cut it, or Gathr
//! ending it for a reason of its own. Read from the turn's status and `why`, which Gathr writes in the words here.

/// Why a turn that was running when the service stopped reads stopped: a restart cut it.
pub const CUT_BY_RESTART: &str = "the service stopped during this turn";

/// Why a turn the usage limit ended after a tool ran stopped: its carry-on waits for the account.
pub const OUT_OF_USAGE: &str = "the account is out of usage";

/// One word for who or what stopped a turn: `you` (the person's stop, which says no why), `restart`, or Gathr's
/// own: `usage`, or `gathr` for anything else. None for a turn that didn't stop.
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
        assert_eq!(read(Some("its chat is archived")), Some("gathr"));
        assert_eq!(word("failed", Some(CUT_BY_RESTART)), None);
        assert_eq!(word("done", None), None);
    }
}
