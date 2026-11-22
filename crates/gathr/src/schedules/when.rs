//! When a schedule runs, on this PC's clock. The service has no time zone library: SQLite has the PC's local time
//! (`'localtime'`, and `'utc'` to turn a local wall-clock time back into an instant), so every local question is
//! asked of it. Instants are UTC microseconds, as everywhere in the database.

use rusqlite::{Connection, params};

use crate::Error;

/// At `minute` of the day on `days` (SQLite's `%w` digits, `0` Sunday), or once at an instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    Days { minute: i64, days: String },
    Once(i64),
}

/// A late run is one that starts more than this after its time.
pub const LATE_US: i64 = 5 * 60 * 1_000_000;

/// What a schedule whose time has come does now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    /// It was due today: it runs now, late or not.
    Run { due: i64, late: bool },
    /// It was due on an earlier day: one skipped run for the latest time missed.
    Skip { due: i64 },
}

/// The PC's wall clock at an instant: its date, the minute of the day and the weekday (`%w`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub date: String,
    pub minute: i64,
    pub weekday: i64,
}

pub fn local(conn: &Connection, us: i64) -> Result<Local, Error> {
    Ok(conn.query_row(
        "SELECT strftime('%Y-%m-%d', ?1, 'unixepoch', 'localtime'),
                CAST(strftime('%H', ?1, 'unixepoch', 'localtime') AS INTEGER) * 60
                    + CAST(strftime('%M', ?1, 'unixepoch', 'localtime') AS INTEGER),
                CAST(strftime('%w', ?1, 'unixepoch', 'localtime') AS INTEGER)",
        [us.div_euclid(1_000_000)],
        |r| {
            Ok(Local {
                date: r.get(0)?,
                minute: r.get(1)?,
                weekday: r.get(2)?,
            })
        },
    )?)
}

/// The instant this PC's clock shows `minute` on the day `plus` days from `date`, and that day's weekday.
pub fn at(conn: &Connection, date: &str, plus: i64, minute: i64) -> Result<(i64, i64), Error> {
    let shift = format!("{plus:+} days");
    let clock = format!("{:02}:{:02}", minute / 60, minute % 60);
    let (secs, weekday): (i64, i64) = conn.query_row(
        "SELECT CAST(strftime('%s', date(?1, ?2) || ' ' || ?3, 'utc') AS INTEGER),
                CAST(strftime('%w', date(?1, ?2)) AS INTEGER)",
        params![date, shift, clock],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((secs * 1_000_000, weekday))
}

impl When {
    /// The first time after `after` it runs; none for a once whose time has passed.
    pub fn next_after(&self, conn: &Connection, after: i64) -> Result<Option<i64>, Error> {
        let (minute, days) = match self {
            When::Once(at) => return Ok((*at > after).then_some(*at)),
            When::Days { minute, days } => (*minute, days),
        };
        let today = local(conn, after)?.date;
        for plus in 0..=8 {
            let (us, weekday) = at(conn, &today, plus, minute)?;
            if us > after && days.contains(&weekday.to_string()) {
                return Ok(Some(us));
            }
        }
        Ok(None)
    }

    /// The last time by `now` it runs: none before its first.
    fn latest_by(&self, conn: &Connection, now: i64) -> Result<Option<i64>, Error> {
        let (minute, days) = match self {
            When::Once(at) => return Ok((*at <= now).then_some(*at)),
            When::Days { minute, days } => (*minute, days),
        };
        let today = local(conn, now)?.date;
        for back in 0..=8 {
            let (us, weekday) = at(conn, &today, -back, minute)?;
            if us <= now && days.contains(&weekday.to_string()) {
                return Ok(Some(us));
            }
        }
        Ok(None)
    }

    /// What it does at `now`, due since `next`: the latest time missed runs when it was today (this PC's date), late
    /// when more than five minutes have passed; one on an earlier day is skipped. Never one run per missed day.
    pub fn due(&self, conn: &Connection, next: i64, now: i64) -> Result<Due, Error> {
        let due = self.latest_by(conn, now)?.filter(|l| *l >= next);
        let due = due.unwrap_or(next);
        Ok(match local(conn, due)?.date == local(conn, now)?.date {
            true => Due::Run {
                due,
                late: now - due > LATE_US,
            },
            false => Due::Skip { due },
        })
    }

    /// In words, made here only and shown as they are: "Weekdays at 9:00", "Once, 23 Nov at 14:00".
    pub fn words(&self, conn: &Connection) -> Result<String, Error> {
        let (minute, days) = match self {
            When::Once(at) => {
                let l = local(conn, *at)?;
                return Ok(format!(
                    "Once, {} at {}",
                    day_month(&l.date),
                    clock(l.minute)
                ));
            }
            When::Days { minute, days } => (*minute, days),
        };
        // Monday first, as a week reads.
        let named: Vec<&str> = MONDAY_FIRST
            .iter()
            .filter(|d| days.contains(&d.to_string()))
            .map(|d| PLURAL[*d as usize])
            .collect();
        let on = match days.len() {
            7 => "Every day".to_string(),
            5 if !days.contains('0') && !days.contains('6') => "Weekdays".to_string(),
            _ => listed(&named),
        };
        Ok(format!("{on} at {}", clock(minute)))
    }
}

const MONDAY_FIRST: [u8; 7] = [1, 2, 3, 4, 5, 6, 0];
const PLURAL: [&str; 7] = [
    "Sundays",
    "Mondays",
    "Tuesdays",
    "Wednesdays",
    "Thursdays",
    "Fridays",
    "Saturdays",
];
const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "Mondays and Thursdays", "Mondays, Wednesdays and Fridays".
fn listed(names: &[&str]) -> String {
    match names.split_last() {
        Some((last, [])) => last.to_string(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// "9:00", "18:30": the 24-hour clock.
pub fn clock(minute: i64) -> String {
    format!("{}:{:02}", minute / 60, minute % 60)
}

/// "23 Nov", from "2026-11-23".
fn day_month(date: &str) -> String {
    let month: usize = date.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(1);
    let day = date.get(8..10).unwrap_or("").trim_start_matches('0');
    format!("{day} {}", MONTHS[month.clamp(1, 12) - 1])
}

/// "Monday 23 Nov, 9:00": a day and time on this PC's clock.
pub fn day_time(conn: &Connection, us: i64) -> Result<String, Error> {
    let l = local(conn, us)?;
    let day = DAYS[l.weekday as usize];
    Ok(format!("{day} {}, {}", day_month(&l.date), clock(l.minute)))
}

/// "Sunday 22 Nov 2026, 14:05": now on this PC's clock, for an agent.
pub fn now_words(conn: &Connection, now: i64) -> Result<String, Error> {
    let l = local(conn, now)?;
    let (day, year) = (DAYS[l.weekday as usize], l.date.get(..4).unwrap_or(""));
    Ok(format!(
        "{day} {} {year}, {}",
        day_month(&l.date),
        clock(l.minute)
    ))
}

/// When the `schedule` tool was asked for.
#[derive(Debug, Default, Clone)]
pub struct Ask {
    pub time: Option<String>,
    pub days: Option<Vec<String>>,
    pub date: Option<String>,
    pub in_minutes: Option<i64>,
}

const GIVE: &str = "Give time as HH:MM on this PC's clock, with days (mon to sun) to repeat on them or date \
                    (YYYY-MM-DD) for once on it; with neither it is once, the next time the clock shows it. Or give \
                    in_minutes (1 to 1440) alone.";

impl Ask {
    /// When it runs, asked at `now`; refused in words that say what to give and what time it is on this PC.
    pub fn when(&self, conn: &Connection, now: i64) -> Result<When, Error> {
        let refuse = |why: &str| -> Result<When, Error> {
            let now = now_words(conn, now)?;
            Err(Error::Refused(format!("{why} It is now {now} on this PC.")))
        };
        if let Some(minutes) = self.in_minutes {
            if self.time.is_some() || self.days.is_some() || self.date.is_some() {
                return refuse(&format!("in_minutes goes alone. {GIVE}"));
            }
            if !(1..=1440).contains(&minutes) {
                return refuse("in_minutes is 1 to 1440.");
            }
            return Ok(When::Once(now + minutes * 60 * 1_000_000));
        }
        let Some(minute) = self.time.as_deref().and_then(minute_of) else {
            return refuse(GIVE);
        };
        match (&self.days, &self.date) {
            (Some(_), Some(_)) => refuse(&format!("Give days or date, not both. {GIVE}")),
            (Some(names), None) => {
                let mut days = String::new();
                for name in names {
                    let Some(d) = weekday_of(name) else {
                        return refuse(&format!(
                            "\"{name}\" isn't a day: give mon, tue, wed, thu, fri, sat or sun."
                        ));
                    };
                    if !days.contains(d) {
                        days.push(d);
                    }
                }
                if days.is_empty() {
                    return refuse(GIVE);
                }
                Ok(When::Days { minute, days })
            }
            (None, Some(date)) => {
                let real: Option<String> =
                    conn.query_row("SELECT date(?1)", [date], |r| r.get(0))?;
                if real.as_deref() != Some(date.as_str()) {
                    return refuse(&format!("\"{date}\" isn't a date: give YYYY-MM-DD."));
                }
                let (at, _) = at(conn, date, 0, minute)?;
                match at > now {
                    true => Ok(When::Once(at)),
                    false => refuse("That time has passed."),
                }
            }
            (None, None) => {
                let today = local(conn, now)?.date;
                let (at, _) = at(conn, &today, 0, minute)?;
                match at > now {
                    true => Ok(When::Once(at)),
                    false => Ok(When::Once(self::at(conn, &today, 1, minute)?.0)),
                }
            }
        }
    }
}

/// "9:00", "09:00", "18:30": the minute of the day.
fn minute_of(time: &str) -> Option<i64> {
    let (h, m) = time.trim().split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m) && time.trim().len() >= 4).then_some(h * 60 + m)
}

/// `mon` to `sun`, or a day's whole name, as SQLite's `%w` digit.
fn weekday_of(name: &str) -> Option<char> {
    let name = name.trim().to_lowercase();
    let at = DAYS
        .iter()
        .position(|day| name.len() >= 3 && day.to_lowercase().starts_with(&name))?;
    char::from_digit(at as u32, 10)
}
