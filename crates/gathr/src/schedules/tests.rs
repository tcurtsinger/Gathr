//! What is due and when the next one is. Every time here is a wall-clock time on this PC, turned into an instant by
//! SQLite as the service does, so the tests hold in any time zone: CI runs in UTC, the owner's PC doesn't. The dates
//! are in late November, away from any clock change.

use rusqlite::Connection;

use super::when::{Ask, Due, When, at};

const MIN: i64 = 60 * 1_000_000;

/// The instant this PC's clock shows `time` on `date`.
fn on(conn: &Connection, date: &str, time: &str) -> i64 {
    let (h, m) = time.split_once(':').unwrap();
    let minute = h.parse::<i64>().unwrap() * 60 + m.parse::<i64>().unwrap();
    at(conn, date, 0, minute).unwrap().0
}

fn weekdays(minute: i64) -> When {
    When::Days {
        minute,
        days: "12345".into(),
    }
}

#[test]
fn a_repeating_schedule_runs_today_late_or_not_and_skips_an_earlier_day_once() {
    let c = Connection::open_in_memory().unwrap();
    // 23 Nov 2026 is a Monday.
    let (mon, tue) = ("2026-11-23", "2026-11-24");
    let nine = weekdays(9 * 60);

    // Made before today's time, it waits for it; made after, for the next weekday's.
    let next = nine.next_after(&c, on(&c, mon, "8:00")).unwrap();
    assert_eq!(next, Some(on(&c, mon, "9:00")));
    let next = nine.next_after(&c, on(&c, mon, "10:00")).unwrap();
    assert_eq!(next, Some(on(&c, tue, "9:00")));
    // Friday's next is Monday's.
    let next = nine.next_after(&c, on(&c, "2026-11-20", "9:30")).unwrap();
    assert_eq!(next, Some(on(&c, mon, "9:00")));

    // Due earlier today: it runs, late only past five minutes.
    let due = on(&c, mon, "9:00");
    let run = nine.due(&c, due, on(&c, mon, "9:04")).unwrap();
    assert_eq!(run, Due::Run { due, late: false });
    let run = nine.due(&c, due, on(&c, mon, "11:40")).unwrap();
    assert_eq!(run, Due::Run { due, late: true });

    // Due Friday and found on Saturday: one skipped run for Friday's time.
    let friday = on(&c, "2026-11-20", "9:00");
    let skipped = nine.due(&c, friday, on(&c, "2026-11-21", "10:00")).unwrap();
    assert_eq!(skipped, Due::Skip { due: friday });

    // Every day, missed since Thursday and found on Sunday morning: one skip, for Saturday, not one per day.
    let daily = When::Days {
        minute: 9 * 60,
        days: "0123456".into(),
    };
    let thursday = on(&c, "2026-11-19", "9:00");
    let skipped = daily
        .due(&c, thursday, on(&c, "2026-11-22", "8:00"))
        .unwrap();
    let saturday = on(&c, "2026-11-21", "9:00");
    assert_eq!(skipped, Due::Skip { due: saturday });
    // Missed since Thursday and found on Sunday after its time: today's runs, late.
    let run = daily
        .due(&c, thursday, on(&c, "2026-11-22", "9:30"))
        .unwrap();
    let sunday = on(&c, "2026-11-22", "9:00");
    assert_eq!(
        run,
        Due::Run {
            due: sunday,
            late: true
        }
    );
}

#[test]
fn a_once_runs_at_its_time_today_and_is_skipped_on_a_later_day() {
    let c = Connection::open_in_memory().unwrap();
    let now = on(&c, "2026-11-23", "10:00");

    // A time alone is once, the next time the clock shows it: tomorrow, as today's has passed.
    let asked = Ask {
        time: Some("9:00".into()),
        ..Ask::default()
    };
    let when = asked.when(&c, now).unwrap();
    assert_eq!(when, When::Once(on(&c, "2026-11-24", "9:00")));
    // Later today, it is today.
    let asked = Ask {
        time: Some("14:00".into()),
        ..Ask::default()
    };
    let once = on(&c, "2026-11-23", "14:00");
    assert_eq!(asked.when(&c, now).unwrap(), When::Once(once));
    let when = When::Once(once);
    assert_eq!(when.next_after(&c, now).unwrap(), Some(once));
    assert_eq!(when.next_after(&c, once).unwrap(), None);
    let run = when.due(&c, once, once + MIN).unwrap();
    assert_eq!(
        run,
        Due::Run {
            due: once,
            late: false
        }
    );
    let skipped = when.due(&c, once, on(&c, "2026-11-24", "8:00")).unwrap();
    assert_eq!(skipped, Due::Skip { due: once });

    // Due two minutes before midnight and found a minute after it: it still runs. Found at three, it is skipped.
    let night = on(&c, "2026-11-23", "23:58");
    let when = When::Once(night);
    let run = when.due(&c, night, on(&c, "2026-11-24", "0:01")).unwrap();
    assert_eq!(
        run,
        Due::Run {
            due: night,
            late: false
        }
    );
    let skipped = when.due(&c, night, on(&c, "2026-11-24", "3:00")).unwrap();
    assert_eq!(skipped, Due::Skip { due: night });

    // in_minutes alone: once, that many minutes from now, and due then.
    let asked = Ask {
        in_minutes: Some(1),
        ..Ask::default()
    };
    let when = asked.when(&c, now).unwrap();
    assert_eq!(when, When::Once(now + MIN));
    assert_eq!(when.next_after(&c, now).unwrap(), Some(now + MIN));
    let run = when.due(&c, now + MIN, now + MIN + 30_000_000).unwrap();
    assert_eq!(
        run,
        Due::Run {
            due: now + MIN,
            late: false
        }
    );

    // A date that has passed, or anything else, is refused with the time now on this PC.
    let asked = Ask {
        time: Some("9:00".into()),
        date: Some("2026-11-23".into()),
        ..Ask::default()
    };
    let refused = asked.when(&c, now).unwrap_err().to_string();
    assert!(
        refused.contains("It is now Monday 23 Nov 2026, 10:00 on this PC."),
        "{refused}"
    );
    let asked = Ask {
        in_minutes: Some(5),
        time: Some("9:00".into()),
        ..Ask::default()
    };
    assert!(asked.when(&c, now).is_err());
}

#[test]
fn when_reads_as_one_phrase() {
    let c = Connection::open_in_memory().unwrap();
    let days = |days: &str, minute| When::Days {
        minute,
        days: days.into(),
    };
    let words = |w: When| w.words(&c).unwrap();
    assert_eq!(words(days("0123456", 18 * 60 + 30)), "Every day at 18:30");
    assert_eq!(words(days("12345", 9 * 60)), "Weekdays at 9:00");
    assert_eq!(words(days("5", 16 * 60)), "Fridays at 16:00");
    assert_eq!(words(days("41", 8 * 60)), "Mondays and Thursdays at 8:00");
    let once = When::Once(on(&c, "2026-11-23", "14:00"));
    assert_eq!(words(once), "Once, 23 Nov at 14:00");
}
