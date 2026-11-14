//! A quiet chat is noticed, and its dead time has a number (0052). One rule for the chat, read
//! from what it waits on (0050): it is stuck when work is owed in it, nothing it waits on moves by
//! itself and nothing waits on the person. Its lead is woken, and the person is told once it has
//! been stuck five minutes in all, each only for what is new to them. Each stuck stretch is an
//! event when it ends.
//!
//! The service checks a chat after every write that may have moved it and at every look. Stuck
//! time is counted from one check to the next, so time the service was down adds nothing.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;
use crate::tasks::Start;
use crate::team::{self, Member, Role};
use crate::waiting::{self, Line, Live, Moves, words};

/// The rule's two times.
#[derive(Debug, Clone, Copy)]
pub struct Times {
    /// The stuck time in all after which the person is told.
    pub tell_after_us: i64,
    /// How far apart the service's looks are. Two checks further apart than two looks had the
    /// service down or the PC asleep between them: that time isn't counted.
    pub look_us: i64,
}

/// The chats with work owed: an agent's task that is neither done nor cancelled, in a chat that
/// isn't archived. Blocked, failed and stopped work counts: its giver was told, and nothing followed.
pub(crate) const WITH_OPEN_WORK: &str =
    "SELECT DISTINCT t.chat_id FROM tasks t JOIN chats c ON c.id = t.chat_id
     WHERE t.state NOT IN ('done', 'cancelled') AND t.owner IS NOT NULL AND c.archived_ts IS NULL";

/// A chat's `quiet` row.
#[derive(Clone, Default, PartialEq)]
struct Kept {
    /// The stretch it is stuck in began; none is open otherwise.
    since: Option<i64>,
    stretch_us: i64,
    /// Stuck time is counted up to here; none while the clock is stopped.
    counted: Option<i64>,
    what: Option<String>,
    /// Stuck time in this count: since the person last wrote, or a stretch they were told in ended.
    /// None while nothing that isn't moving is new to the person: no time is counted then.
    total_us: Option<i64>,
    /// The lead's wake and the telling of this count.
    wake_turn: Option<String>,
    told: Option<i64>,
    /// The person's message the count runs from.
    reset: i64,
    /// The keys of what wasn't moving when the person was last told, and of the lead's news at its
    /// last wake. They outlast the count: the person's message makes nothing new.
    told_what: Option<String>,
    woke_what: Option<String>,
}

impl Kept {
    fn read(tx: &Transaction, chat: &str) -> Result<Kept, Error> {
        let mut stmt = tx.prepare_cached(
            "SELECT since_ts, stretch_us, counted_ts, what, total_us, wake_turn, told_ts, reset_ts,
                    told_what, woke_what
             FROM quiet WHERE chat_id = ?1",
        )?;
        let row = stmt.query_row([chat], |r| {
            Ok(Kept {
                since: r.get(0)?,
                stretch_us: r.get(1)?,
                counted: r.get(2)?,
                what: r.get(3)?,
                total_us: r.get(4)?,
                wake_turn: r.get(5)?,
                told: r.get(6)?,
                reset: r.get(7)?,
                told_what: r.get(8)?,
                woke_what: r.get(9)?,
            })
        });
        Ok(row.optional()?.unwrap_or_default())
    }

    fn save(&self, tx: &Transaction, chat: &str) -> Result<(), Error> {
        tx.prepare_cached(
            "INSERT OR REPLACE INTO quiet VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?
        .execute(params![
            chat,
            self.since,
            self.stretch_us,
            self.counted,
            self.what,
            self.total_us,
            self.wake_turn,
            self.told,
            self.reset,
            self.told_what,
            self.woke_what
        ])?;
        Ok(())
    }

    /// Ends the open stretch, if one is: its dead time is an event, with what wasn't moving.
    fn end(&mut self, tx: &Transaction, chat: &str, by: &str, trace: &str) -> Result<(), Error> {
        let Some(since) = self.since.take() else {
            return Ok(());
        };
        let what = self.what.take().map(|w| serde_json::from_str::<Value>(&w));
        let what = what.transpose()?;
        let mut event = Event::point("gathr.quiet.stretch").trace(trace).chat(chat);
        event.duration_us = Some(self.stretch_us);
        let event = event.ok().attr("since_us", since).attr("ended_by", by);
        let event = event.attr("woke", self.wake_turn.is_some());
        let event = event.attr("told", self.told.is_some()).attr("what", what);
        event.insert(tx)?;
        self.stretch_us = 0;
        self.counted = None;
        Ok(())
    }

    fn count_again(&mut self) {
        self.total_us = None;
        self.wake_turn = None;
        self.told = None;
    }
}

fn lead(conn: &Connection, chat: &str) -> Result<Option<Member>, Error> {
    let members = team::members(conn, chat)?;
    Ok(members.into_iter().find(|m| m.role == Role::Supervisor))
}

/// What a line is of, whatever its words say. One thing that isn't moving keeps its key from check
/// to check, and when its giver hears of it; a task that stopped again has a new one, by its time.
fn key(l: &Line) -> Value {
    json!([l.kind, l.task, l.on, l.agent, l.since_us])
}

/// The keys kept in a column of the chat's row.
fn keys(kept: &Option<String>) -> Result<Vec<Value>, Error> {
    Ok(match kept {
        Some(keys) => serde_json::from_str(keys)?,
        None => Vec::new(),
    })
}

fn keys_of(lines: &[&Line]) -> Option<String> {
    let keys: Vec<Value> = lines.iter().map(|l| key(l)).collect();
    Some(Value::from(keys).to_string())
}

/// The lines with the new ones first, each part in the order it had.
fn new_first<'a>(lines: &[&'a Line], new: impl Fn(&&Line) -> bool) -> Vec<&'a Line> {
    let mut lines = lines.to_vec();
    lines.sort_by_key(|l| !new(l));
    lines
}

/// Looks at one chat, with what the running service knows of it, at the clock `now_us`: counts its
/// stuck time, ends its stretch when it moves or the person wrote, and when it is stuck wakes its
/// lead and tells the person, each only for what is new to them. The turns to start.
pub fn check(
    conn: &mut Connection,
    chat: &str,
    live: &Live,
    now_us: i64,
    times: Times,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let was = Kept::read(&tx, chat)?;
    let open: bool = tx
        .prepare_cached(&format!(
            "SELECT EXISTS ({WITH_OPEN_WORK} AND t.chat_id = ?1)"
        ))?
        .query_row([chat], |r| r.get(0))?;
    if !open && was.since.is_none() {
        return Ok(Vec::new());
    }
    let mut kept = was.clone();
    if let (Some(_), Some(counted)) = (kept.since, kept.counted) {
        let gap = now_us - counted;
        if (0..=2 * times.look_us).contains(&gap) {
            kept.stretch_us += gap;
            // Told, the count rests: it starts again when that stretch ends.
            if let (None, Some(total)) = (kept.told, kept.total_us.as_mut()) {
                *total += gap;
            }
        }
    }
    let wrote: Option<i64> = tx
        .prepare_cached("SELECT max(ts) FROM messages WHERE chat_id = ?1 AND role = 'user'")?
        .query_row([chat], |r| r.get(0))?;
    if let Some(wrote) = wrote.filter(|ts| *ts > kept.reset) {
        kept.end(&tx, chat, "wrote", trace)?;
        kept.count_again();
        kept.reset = wrote;
    }

    let answer = waiting::answer(&tx, chat, live, now_us)?;
    // The lead's wake doesn't end the stretch it was woken in. One that waits on the person, or
    // runs with no progress, is a line like any other.
    let wake_moves =
        |l: &Line| kept.wake_turn.is_some() && l.turn == kept.wake_turn && l.moves == Moves::Itself;
    let wake_runs = answer
        .lines
        .iter()
        .any(|l| l.kind == "turn" && wake_moves(l));
    let lines: Vec<&Line> = answer.lines.iter().filter(|l| !wake_moves(l)).collect();
    let moving = |l: &&Line| matches!(l.moves, Moves::Itself | Moves::You);
    if !open || lines.iter().any(moving) {
        if kept.since.is_some() {
            let told = kept.told.is_some();
            kept.end(&tx, chat, "moved", trace)?;
            if told {
                kept.count_again();
            }
        }
        return saved(tx, chat, &was, &kept, Vec::new());
    }

    // What nobody is on is what isn't moving, and the rest only follows it. With nothing of
    // nobody's (trouble its giver is yet to hear of), it is all there is.
    let nobodys = |l: &&Line| l.moves == Moves::Nobody;
    let mut not_moving = lines.clone();
    if not_moving.iter().any(nobodys) {
        not_moving.retain(nobodys);
    }
    kept.since = kept.since.or(Some(now_us));
    // While the wake runs and makes progress the clock is stopped.
    kept.counted = (!wake_runs).then_some(now_us);
    let what: Vec<&str> = not_moving.iter().map(|l| l.what.as_str()).collect();
    kept.what = Some(Value::from(what).to_string());

    let mut starts = Vec::new();
    // Trouble its giver has heard is news to nobody; unheard, in a chat where nothing moves, no
    // hear-back is on its way. Of a member's silent turn its lead was told when it went silent.
    let news = |l: &&Line| match l.moves {
        Moves::Nobody => l.kind != "trouble" && l.silent_us.is_none(),
        _ => l.kind == "trouble",
    };
    let woke = keys(&kept.woke_what)?;
    let fresh = |l: &&Line| news(l) && !woke.contains(&key(l));
    // One wake at a time: what is new while it is queued or runs wakes the lead once it is over.
    let waking = kept.wake_turn.is_some() && answer.lines.iter().any(|l| l.turn == kept.wake_turn);
    let lead = match !waking && lines.iter().any(fresh) {
        true => lead(&tx, chat)?,
        false => None,
    };
    // Behind the lead's own silent turn a wake would only queue.
    let hangs =
        |l: &&Line| l.silent_us.is_some() && l.agent.as_ref() == lead.as_ref().map(|m| &m.agent);
    if let Some(lead) = lead.as_ref().filter(|_| !lines.iter().any(hangs)) {
        // Named in its headline too: unheard trouble is new to it, though it isn't nobody's.
        let all = new_first(&lines, fresh);
        let named = |l: &&Line| fresh(l) || not_moving.contains(l);
        let named: Vec<&Line> = all.iter().copied().filter(named).collect();
        let text = words::wake(&named, &all, &answer.agents, now_us);
        let start = Start::queue(&tx, (chat, &lead.agent), (None, None), text)?;
        let event = Event::point("gathr.quiet.woke").trace(trace).chat(chat);
        let event = event.ok().attr("gen_ai.agent.id", lead.agent.clone());
        event.attr("turn", start.turn.clone()).insert(&tx)?;
        kept.wake_turn = Some(start.turn.clone());
        let news: Vec<&Line> = lines.iter().copied().filter(news).collect();
        kept.woke_what = keys_of(&news);
        starts.push(start);
    }
    let told = keys(&kept.told_what)?;
    let untold = |l: &&Line| !told.contains(&key(l));
    if kept.told.is_none() {
        if !not_moving.iter().any(untold) {
            // No time counts for what they were told of: a new thing gets its own five minutes.
            kept.total_us = None;
        } else if *kept.total_us.get_or_insert(0) >= times.tell_after_us {
            let line = words::stuck(&new_first(&not_moving, untold), &answer.agents, now_us);
            crate::store::say(&tx, chat, &line)?;
            let event = Event::point("gathr.quiet.told").trace(trace).chat(chat);
            event.ok().attr("stuck_us", kept.total_us).insert(&tx)?;
            kept.told = Some(now_us);
            kept.told_what = keys_of(&not_moving);
        }
    }
    saved(tx, chat, &was, &kept, starts)
}

/// Keeps the chat's row when the check changed it, and commits.
fn saved(
    tx: Transaction,
    chat: &str,
    was: &Kept,
    kept: &Kept,
    starts: Vec<Start>,
) -> Result<Vec<Start>, Error> {
    if kept != was {
        kept.save(&tx, chat)?;
    }
    tx.commit()?;
    Ok(starts)
}

/// The chats to check at a look: those with work owed, and those whose stretch is open still.
pub fn chats(conn: &Connection) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "{WITH_OPEN_WORK} UNION SELECT chat_id FROM quiet WHERE since_ts IS NOT NULL"
    ))?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// A turn of `agent`'s went silent: its chat's lead is told, in a turn on no task, and can write
/// to that agent. The service calls this once for a turn. A lead's own silent turn tells nobody,
/// and nor does one in a chat with no lead: `check` covers both.
pub fn silent_turn(
    conn: &mut Connection,
    chat: &str,
    agent: &str,
    task: Option<i64>,
    quiet_ms: i64,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let Some(lead) = lead(&tx, chat)?.filter(|lead| lead.agent != agent) else {
        return Ok(Vec::new());
    };
    let name = crate::tasks::name_of(&tx, agent)?;
    let text = words::silent(&name, task, quiet_ms);
    let start = Start::queue(&tx, (chat, &lead.agent), (None, None), text)?;
    let event = Event::point("gathr.turn.silent_told").trace(trace);
    let event = event.chat(chat).ok().attr("gen_ai.agent.id", agent);
    let event = event.attr("lead", lead.agent).attr("quiet_ms", quiet_ms);
    event.insert(&tx)?;
    tx.commit()?;
    Ok(vec![start])
}
