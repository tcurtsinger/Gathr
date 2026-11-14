//! 0052: a quiet chat is noticed, and its dead time has a number. Scenarios 6c and 6h, and the
//! rule's half of 6d, on a real database: `quiet::check` as the service calls it, with the clock
//! passed in and what only the running service knows built by hand.

mod common;

use std::collections::HashMap;

use common::TempDir;
use gathr::quiet::{self, Times};
use gathr::tasks::{self, Finish, Give, Kind, Outcome, Said, Start};
use gathr::waiting::Live;
use gathr::{landings, pauses, store};
use rusqlite::{Connection, OptionalExtension, types::FromSql};
use serde_json::{Value, json};

const T: &str = "test-trace";
/// A second.
const S: i64 = 1_000_000;
/// The rule's times as the service has them: the person is told after 5 minutes, a look every 2.
const TIMES: Times = Times {
    tell_after_us: 300 * S,
    look_us: 120 * S,
};

/// A project's chat `c` with a Supervisor (Lead), Ann, Bo and Cy, and a chat `solo` of Cy's alone,
/// which has no Supervisor.
fn team(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1),
             ('bo', 'Bo', 'bo', 'acc', 1), ('cy', 'Cy', 'cy', 'acc', 1);
         INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Site', 'site', 'x', 1);
         INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'Chat', 1, 'p'), ('solo', 'Solo', 1, 'p');
         INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1),
             ('c', 'bo', 'member', 1), ('c', 'cy', 'member', 1), ('solo', 'cy', 'member', 1);",
    )
    .unwrap();
    conn
}

/// The service as it runs, knowing nothing more than the database.
fn service() -> Live {
    Live {
        running: true,
        ..Live::default()
    }
}

/// The service with one running turn that has made no progress since `since`.
fn silent(turn: &str, since: i64) -> Live {
    Live {
        silent: HashMap::from([(turn.to_string(), since)]),
        ..service()
    }
}

fn check(conn: &mut Connection, live: &Live, now: i64) -> Vec<Start> {
    quiet::check(conn, "c", live, now, TIMES, T).unwrap()
}

/// A task Lead gave `owner`: ready, with the turn Gathr queued on it.
fn give(conn: &mut Connection, owner: &str) -> i64 {
    let g = Give {
        chat: "c",
        giver: Some("lead"),
        owner,
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix the build",
        why: None,
        after: &[],
        pr: None,
        in_passing: false,
    };
    tasks::give(conn, &g, T).unwrap().0.number
}

/// A task of `owner`'s that is ready with no turn on it: work is owed, and nobody is on it.
fn idle(conn: &mut Connection, owner: &str) -> i64 {
    let task = give(conn, owner);
    conn.execute("DELETE FROM turns WHERE task = ?1", [task])
        .unwrap();
    task
}

/// The turn Gathr queued on the task, running now.
fn begin(conn: &mut Connection, task: i64) -> String {
    let sql = "SELECT id FROM turns WHERE task = ?1 AND status = 'queued' ORDER BY rowid LIMIT 1";
    let id: String = conn.query_row(sql, [task], |r| r.get(0)).unwrap();
    store::set_turn(conn, &id, "running", None).unwrap();
    tasks::started(conn, task, T).unwrap();
    id
}

/// The task's queued turn runs and fails: the task is failed, and its giver hasn't heard.
fn fail(conn: &mut Connection, task: i64) {
    let turn = begin(conn, task);
    store::set_turn(conn, &turn, "failed", Some("boom")).unwrap();
    tasks::ended(conn, task, Outcome::Failed("boom".into()), T).unwrap();
}

fn turn(conn: &Connection, id: &str, agent: &str, status: &str) {
    store::add_turn(conn, "c", id, T, (Some(agent), None, None)).unwrap();
    store::set_turn(conn, id, status, None).unwrap();
}

fn end(conn: &Connection, turn: &str) {
    store::set_turn(conn, turn, "done", None).unwrap();
}

/// A check, with the lead's wake over at once: it changes nothing. How many were woken.
fn look(conn: &mut Connection, now: i64) -> usize {
    let woke = check(conn, &service(), now);
    for wake in &woke {
        end(conn, &wake.turn);
    }
    woke.len()
}

/// The person writes in the chat, and the turn their message started is over.
fn writes(conn: &Connection, id: &str) {
    turn(conn, id, "lead", "running");
    store::add_message(conn, "c", id, "user", "text", "Any news?").unwrap();
    end(conn, id);
}

/// One column of the chat's `quiet` row: none when there is no row, or nothing in it.
fn kept<V: FromSql>(conn: &Connection, column: &str) -> Option<V> {
    let sql = format!("SELECT {column} FROM quiet WHERE chat_id = 'c'");
    let found = conn.query_row(&sql, [], |r| r.get::<_, Option<V>>(0));
    found.optional().unwrap().flatten()
}

/// Whether the chat is in a stuck stretch now.
fn stuck(conn: &Connection) -> bool {
    kept::<i64>(conn, "since_ts").is_some()
}

fn total(conn: &Connection) -> i64 {
    kept(conn, "total_us").unwrap_or(0)
}

/// What Gathr said to the person in the chat, in no turn.
fn said(conn: &Connection) -> Vec<String> {
    let sql = "SELECT body FROM messages WHERE chat_id = 'c' AND turn_id IS NULL AND role = 'gathr' ORDER BY ts";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.collect::<Result<_, _>>().unwrap()
}

fn needs(conn: &Connection) -> bool {
    store::chat_state(conn, "c").unwrap().unwrap().1
}

/// The events of one name, oldest first: each one's duration and attributes.
fn events(conn: &Connection, name: &str) -> Vec<(Option<i64>, Value)> {
    let sql = "SELECT duration_us, json(attrs) FROM events WHERE name = ?1 ORDER BY rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([name], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))
        .unwrap();
    rows.map(|row| {
        let (duration, attrs) = row.unwrap();
        (duration, serde_json::from_str(&attrs).unwrap())
    })
    .collect()
}

/// The stretches that ended: each one's length and what ended it.
fn stretches(conn: &Connection) -> Vec<(i64, String)> {
    let ended = events(conn, "gathr.quiet.stretch");
    let each = ended.into_iter().map(|(length, attrs)| {
        (
            length.unwrap(),
            attrs["ended_by"].as_str().unwrap().to_string(),
        )
    });
    each.collect()
}

/// Scenario 6c: work owed and nothing moving. The lead is woken once; at 5 minutes in all the
/// person is told and the chat needs them; the stretch is recorded with its length when it ends.
#[test]
fn a_stuck_chat_wakes_its_lead_once_then_tells_the_person_at_five_minutes() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    assert!(check(&mut conn, &live, gathr::ids::now_us()).is_empty());
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM quiet", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0, "no work is owed: nothing is kept");
    assert_eq!(quiet::chats(&conn).unwrap(), Vec::<String>::new());

    let task = idle(&mut conn, "ann");
    let line = format!("- #{task} (Ann) is ready, and no turn is on it");
    assert_eq!(quiet::chats(&conn).unwrap(), ["c"]);
    let t0 = gathr::ids::now_us();
    let woke = check(&mut conn, &live, t0);
    assert_eq!(woke.len(), 1, "the lead is woken");
    assert_eq!((woke[0].agent.as_str(), woke[0].task), ("lead", None));
    let wake = format!(
        "Not moving: #{task} (Ann). Nothing waits on the person.\n{line}, for under a minute\n\
         Move it on: give, restart or cancel work, or ask the person. If nothing can be done, say why."
    );
    assert_eq!(woke[0].text, wake);
    assert!(stuck(&conn));
    // The lead's turn runs and changes nothing.
    end(&conn, &woke[0].turn);
    for seconds in [100, 200] {
        let again = check(&mut conn, &live, t0 + seconds * S);
        assert!(again.is_empty(), "woken once in a stretch");
        assert_eq!(said(&conn), Vec::<String>::new(), "at {seconds} s");
    }
    assert!(!needs(&conn));

    assert!(check(&mut conn, &live, t0 + 300 * S).is_empty());
    let told = format!(
        "Not moving for 5 min: #{task} (Ann). Write here to move it on.\n{line}, for 5 min"
    );
    assert_eq!(said(&conn), [told], "told at 5 minutes in all");
    assert!(needs(&conn), "the chat needs the person");
    assert!(check(&mut conn, &live, t0 + 400 * S).is_empty());
    assert_eq!(said(&conn).len(), 1, "told once");

    // A turn on the task: it moves by itself again, and the stretch is an event with its length.
    store::add_turn(&conn, "c", "work", T, (Some("ann"), Some(task), None)).unwrap();
    store::set_turn(&conn, "work", "running", None).unwrap();
    assert!(check(&mut conn, &live, t0 + 410 * S).is_empty());
    let ended = events(&conn, "gathr.quiet.stretch");
    assert_eq!(ended.len(), 1, "{ended:?}");
    let (length, attrs) = &ended[0];
    assert_eq!(*length, Some(410 * S));
    assert_eq!(
        attrs,
        &json!({ "since_us": t0, "ended_by": "moved", "woke": true, "told": true,
            "what": [format!("#{task} (Ann) is ready, and no turn is on it")] })
    );
    assert!(!stuck(&conn));
    assert!(!needs(&conn), "the mark goes when the stretch ends");
    assert_eq!(total(&conn), 0, "told: the count starts again");
}

/// Scenario 6h: a lead's wake that changes nothing isn't repeated, stretch after stretch, while
/// what isn't moving is the same thing, and the person is still told at 5 minutes. The clock stops
/// only while the wake runs and makes progress.
#[test]
fn a_wake_that_changes_nothing_isnt_repeated_and_the_person_is_still_told() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    idle(&mut conn, "ann");
    let t0 = gathr::ids::now_us();
    let woke = check(&mut conn, &live, t0);
    assert_eq!(woke.len(), 1);
    let wake = woke[0].turn.clone();

    // It was queued for a minute, which counts; running, with progress, it stops the clock.
    store::set_turn(&conn, &wake, "running", None).unwrap();
    assert!(check(&mut conn, &live, t0 + 60 * S).is_empty());
    assert_eq!(total(&conn), 60 * S);
    assert!(check(&mut conn, &live, t0 + 120 * S).is_empty());
    assert_eq!(total(&conn), 60 * S, "the wake runs: no time is added");

    // Running with no progress, it isn't moving anything: the clock runs again.
    let hung = silent(&wake, t0 + 60 * S);
    assert!(check(&mut conn, &hung, t0 + 180 * S).is_empty());
    assert!(check(&mut conn, &hung, t0 + 240 * S).is_empty());
    assert_eq!(total(&conn), 120 * S, "the wake is silent: time is added");

    // Waiting for the person to sign in, the chat waits on them: the stretch ends, nothing is said.
    store::set_turn(&conn, &wake, "waiting_sign_in", None).unwrap();
    assert!(check(&mut conn, &live, t0 + 250 * S).is_empty());
    assert_eq!(stretches(&conn), [(130 * S, "moved".to_string())]);
    assert_eq!(said(&conn), Vec::<String>::new());
    assert_eq!(total(&conn), 130 * S, "not told: the count goes on");

    // The lead's turn ends having written to a member, whose turn changes nothing: three times.
    end(&conn, &wake);
    let mut now = t0 + 250 * S;
    for round in 0..3 {
        let asked = format!("asked-{round}");
        turn(&conn, &asked, "ann", "running");
        now += 10 * S;
        assert!(check(&mut conn, &live, now).is_empty());
        assert!(!stuck(&conn), "Ann's turn runs");
        end(&conn, &asked);
        now += 10 * S;
        assert!(check(&mut conn, &live, now).is_empty(), "woken once");
        assert!(stuck(&conn), "it changed nothing");
        now += 10 * S;
        assert!(check(&mut conn, &live, now).is_empty(), "woken once");
    }
    assert_eq!(total(&conn), 180 * S);
    assert!(check(&mut conn, &live, now + 100 * S).is_empty());
    assert_eq!(said(&conn), Vec::<String>::new(), "280 s in all");
    assert!(check(&mut conn, &live, now + 120 * S).is_empty());
    assert_eq!(said(&conn).len(), 1, "told at 5 minutes in all");
    let wakes: i64 = conn
        .query_row(
            "SELECT count(*) FROM turns WHERE agent_id = 'lead'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wakes, 1, "one wake for the one task");
}

/// Scenario 6h: a lead that already heard the trouble isn't woken again, and the person is still
/// told. Trouble nobody is about to hear of is news.
#[test]
fn a_lead_hears_of_trouble_once() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    let task = give(&mut conn, "ann");
    fail(&mut conn, task);
    conn.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [task])
        .unwrap();
    let t0 = gathr::ids::now_us();
    assert!(
        check(&mut conn, &live, t0).is_empty(),
        "its giver has heard: nobody has news"
    );
    assert!(stuck(&conn));
    assert!(check(&mut conn, &live, t0 + 150 * S).is_empty());
    assert!(check(&mut conn, &live, t0 + 300 * S).is_empty());
    let told = format!(
        "Not moving for 5 min: #{task} (Ann). Write here to move it on.\n\
         - #{task} (Ann) is failed: boom. Lead was told, for 5 min"
    );
    assert_eq!(said(&conn), [told]);

    // Stopped by a restart of the service, say: its giver hasn't heard, and no hear-back is queued.
    conn.execute("UPDATE tasks SET heard = 0 WHERE number = ?1", [task])
        .unwrap();
    let woke = check(&mut conn, &live, t0 + 310 * S);
    assert_eq!(woke.len(), 1, "unheard trouble is news");
    assert_eq!((woke[0].agent.as_str(), woke[0].task), ("lead", None));
    let news = format!("- #{task} (Ann) is failed: boom. Lead hears of it next");
    assert!(woke[0].text.contains(&news), "{}", woke[0].text);
}

/// Stuck before the change and not after it: the stretch is an event, ended because something
/// moves or waits on the person.
fn ends(conn: &mut Connection, now: &mut i64, what: &str, change: impl FnOnce(&mut Connection)) {
    *now += 5 * S;
    check(conn, &service(), *now);
    assert!(stuck(conn), "stuck before {what}");
    let before = stretches(conn).len();
    change(conn);
    *now += 5 * S;
    check(conn, &service(), *now);
    assert!(!stuck(conn), "{what}");
    let all = stretches(conn);
    assert_eq!(all.len(), before + 1, "{what}: {all:?}");
    assert_eq!(all[before].1, "moved", "{what}");
}

/// Scenario 6c: the lead is the member whose role says so. A chat with none wakes nobody, and the
/// person is still told.
#[test]
fn a_chat_with_no_lead_wakes_nobody_and_the_person_is_still_told() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, owner, kind, what, state, created_ts, updated_ts)
         VALUES ('solo', 'cy', 'work', 'Fix the build', 'ready', 5, 5)",
        [],
    )
    .unwrap();
    let t0 = gathr::ids::now_us();
    for seconds in [0, 150, 300] {
        let now = t0 + seconds * S;
        let woke = quiet::check(&mut conn, "solo", &service(), now, TIMES, T).unwrap();
        assert!(woke.is_empty(), "Cy is a member, not a Supervisor");
    }
    let sql = "SELECT count(*) FROM messages WHERE chat_id = 'solo' AND role = 'gathr'";
    let told: i64 = conn.query_row(sql, [], |r| r.get(0)).unwrap();
    assert_eq!(told, 1, "told at 5 minutes");
}

/// Scenario 6h: never told as stuck. A chat that waits on the person, a chat with one agent
/// paused, a paused chat, a chat waiting on a queued CI job and an archived chat each end the
/// stretch, and nothing is said.
#[test]
fn what_waits_on_the_person_or_moves_by_itself_ends_a_stretch() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    idle(&mut conn, "ann");
    let mut now = gathr::ids::now_us();
    let woke = check(&mut conn, &live, now);
    assert_eq!(woke.len(), 1, "stuck: the lead is woken");
    end(&conn, &woke[0].turn);

    ends(&mut conn, &mut now, "a question to the person", |conn| {
        conn.execute(
            "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts)
             VALUES ('c', 'lead', 'person', 'Pick a colour', 'ready', 5, 5)",
            [],
        )
        .unwrap();
    });
    conn.execute("UPDATE tasks SET state = 'done' WHERE kind = 'person'", [])
        .unwrap();

    ends(&mut conn, &mut now, "a pause on the task's owner", |conn| {
        pauses::pause(conn, "agent", "ann", false, T).unwrap();
    });
    pauses::resume(&conn, "agent", "ann", T).unwrap();

    ends(&mut conn, &mut now, "a pause on the chat", |conn| {
        pauses::pause(conn, "chat", "c", false, T).unwrap();
    });
    pauses::resume(&conn, "chat", "c", T).unwrap();

    ends(&mut conn, &mut now, "a wait on a CI job", |conn| {
        let leave = gathr::waits::Leave {
            chat: "c",
            agent: "bo",
            task: None,
            left_in: None,
            cwd: Some("."),
            then_do: "merge",
            parts: vec![gathr::waits::Part {
                kind: "ci".into(),
                target: "157".into(),
                repo: None,
            }],
        };
        let wait = gathr::waits::leave(conn, &leave, T).unwrap();
        let queued = "0 of 1 checks on #157 at 0123456 finished; queued: UI (macOS)";
        gathr::waits::looked(conn, &wait.id, None, Some(queued), None).unwrap();
    });
    conn.execute("UPDATE waits SET state = 'cancelled'", [])
        .unwrap();

    // A pause on a member with nothing in the chat holds nothing in it.
    now += 5 * S;
    check(&mut conn, &live, now);
    pauses::pause(&conn, "agent", "cy", false, T).unwrap();
    now += 5 * S;
    check(&mut conn, &live, now);
    assert!(stuck(&conn), "Cy has nothing here");
    assert_eq!(stretches(&conn).len(), 4);
    pauses::resume(&conn, "agent", "cy", T).unwrap();

    ends(&mut conn, &mut now, "archiving the chat", |conn| {
        gathr::archive::archive(conn, "c", T).unwrap();
        let open = quiet::chats(conn).unwrap();
        assert_eq!(open, ["c"], "its stretch is open until the next check");
    });
    assert_eq!(quiet::chats(&conn).unwrap(), Vec::<String>::new());
    assert_eq!(said(&conn), Vec::<String>::new(), "never told as stuck");
    // What is kept of a chat is deleted with it.
    gathr::archive::delete(&mut conn, "c", T).unwrap();
    assert_eq!(kept::<i64>(&conn, "reset_ts"), None);
}

/// Scenario 6h: after a restart the lead isn't woken twice and the time down isn't counted. Time
/// is counted check by check, so an hour with no check, the service down or the PC asleep, adds
/// nothing.
#[test]
fn time_down_or_asleep_isnt_counted() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    idle(&mut conn, "ann");
    let t0 = gathr::ids::now_us();
    let woke = check(&mut conn, &live, t0);
    assert_eq!(woke.len(), 1);
    end(&conn, &woke[0].turn);
    assert!(check(&mut conn, &live, t0 + 60 * S).is_empty());
    assert_eq!(total(&conn), 60 * S);

    let later = t0 + 3660 * S;
    assert!(check(&mut conn, &live, later).is_empty(), "woken once");
    assert_eq!(total(&conn), 60 * S, "an hour with no check");
    assert_eq!(kept::<i64>(&conn, "stretch_us"), Some(60 * S));
    assert_eq!(said(&conn), Vec::<String>::new());

    assert!(check(&mut conn, &live, later + 60 * S).is_empty());
    assert_eq!(total(&conn), 120 * S, "the next gap counts");
}

/// Scenario 6h: a task the lead restarts that fails again, three times over, still gets the person
/// told once 5 minutes of stuck time have added up: a stretch that ends because something moved
/// doesn't set the count back. Each failure is new: with no hear-back coming, the lead is woken
/// for each, once.
#[test]
fn work_that_fails_again_and_again_still_reaches_the_person() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = give(&mut conn, "ann");
    let mut now = gathr::ids::now_us();
    let mut woken = 0;
    for round in 1..=3 {
        fail(&mut conn, task);
        woken += look(&mut conn, now);
        assert!(stuck(&conn), "round {round}");
        // Two minutes with nothing moving.
        for _ in 0..2 {
            now += 60 * S;
            woken += look(&mut conn, now);
        }
        if round < 3 {
            assert_eq!(said(&conn), Vec::<String>::new(), "round {round}");
            tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
            woken += look(&mut conn, now);
            assert!(!stuck(&conn), "restarted: its turn is queued");
        }
    }
    assert_eq!(woken, 3, "each failure is news to the lead, once");
    let told = said(&conn);
    assert_eq!(told.len(), 1, "told in the third stretch: {told:?}");
    let before: Vec<i64> = stretches(&conn).into_iter().map(|s| s.0).collect();
    assert_eq!(before, [120 * S, 120 * S], "the two stretches before");
}

/// Scenario 6c: told once in a stretch, however long it lasts, and no time counts while the
/// person is told. Their message ends the stretch, even into a turn that stays silent, and of that
/// turn they aren't told again.
#[test]
fn once_told_the_person_isnt_told_again_in_that_stretch() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = give(&mut conn, "ann");
    let running = begin(&mut conn, task);
    let t0 = gathr::ids::now_us();
    let live = silent(&running, t0);
    for seconds in [0, 150, 300] {
        assert!(check(&mut conn, &live, t0 + seconds * S).is_empty());
    }
    assert_eq!(said(&conn).len(), 1, "told");
    assert_eq!(total(&conn), 300 * S);

    let mut now = t0 + 300 * S;
    while now < t0 + 8 * 3600 * S {
        now += 200 * S;
        assert!(check(&mut conn, &live, now).is_empty());
    }
    assert_eq!(said(&conn).len(), 1, "8 hours on: told once");
    assert_eq!(total(&conn), 300 * S, "no time counts while they are told");
    assert!(needs(&conn));

    store::add_message(&conn, "c", &running, "user", "text", "Any news?").unwrap();
    assert!(check(&mut conn, &live, now + S).is_empty());
    let ended = events(&conn, "gathr.quiet.stretch");
    assert_eq!(ended.len(), 1, "{ended:?}");
    assert_eq!(ended[0].0, Some(now + S - t0), "its whole length");
    assert_eq!(
        (&ended[0].1["ended_by"], &ended[0].1["told"]),
        (&json!("wrote"), &json!(true))
    );
    assert!(!needs(&conn), "the mark goes with the stretch");
    assert!(stuck(&conn), "the turn is still silent: a new stretch");
    assert_eq!(total(&conn), 0, "and a new count");

    // The new count holds no time: they were told of this turn.
    for seconds in [200, 400, 600] {
        assert!(check(&mut conn, &live, now + seconds * S).is_empty());
    }
    assert_eq!(said(&conn).len(), 1, "10 minutes on: the same turn");
    assert_eq!(total(&conn), 0, "no time counts for what they were told of");
    assert!(!needs(&conn));
}

/// Scenario 6c: the person is told again only of what is new. Told of one task, they write, and it
/// still isn't moving: nothing more is said, for hours. A second thing that stops moving gets its
/// own five minutes, and the line then names it first. A task that is restarted and fails again is
/// a new thing; one whose giver has now heard of it is not.
#[test]
fn the_person_is_told_again_only_of_what_is_new() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let first = idle(&mut conn, "ann");
    let t0 = gathr::ids::now_us();
    for seconds in [0, 150, 300] {
        look(&mut conn, t0 + seconds * S);
    }
    assert_eq!(said(&conn).len(), 1, "told of the first");
    assert!(needs(&conn));

    writes(&conn, "asked");
    let mut now = t0 + 300 * S;
    while now < t0 + 3 * 3600 * S {
        now += 100 * S;
        look(&mut conn, now);
        assert!(stuck(&conn), "still not moving");
    }
    let told = said(&conn);
    assert_eq!(told.len(), 1, "3 hours on, the same list: {told:?}");
    assert!(!needs(&conn), "and the chat doesn't need them");

    // A task that stops and is cancelled before they are told of it leaves no time behind.
    let passing = idle(&mut conn, "cy");
    for _ in 0..3 {
        now += 100 * S;
        look(&mut conn, now);
    }
    assert_eq!(total(&conn), 200 * S, "time counts for a new thing");
    conn.execute(
        "UPDATE tasks SET state = 'cancelled' WHERE number = ?1",
        [passing],
    )
    .unwrap();
    now += 50 * S;
    look(&mut conn, now);
    assert_eq!(total(&conn), 0, "nothing here is new");

    // A second task nobody is on, first seen 100 s after the check before.
    let second = idle(&mut conn, "bo");
    now += 100 * S;
    conn.execute(
        "UPDATE tasks SET updated_ts = ?2 WHERE number = ?1",
        [second, now],
    )
    .unwrap();
    for seconds in [0, 100, 200, 299] {
        look(&mut conn, now + seconds * S);
        assert_eq!(said(&conn).len(), 1, "{seconds} s of the second");
    }
    assert!(!needs(&conn));
    look(&mut conn, now + 300 * S);
    let told = said(&conn);
    assert_eq!(told.len(), 2, "five minutes of the second: {told:?}");
    let lines: Vec<&str> = told[1].lines().collect();
    let ready = "is ready, and no turn is on it, for";
    assert_eq!(
        lines[..2],
        [
            format!("Not moving for 5 min: #{second} (Bo) and 1 more. Write here to move it on."),
            format!("- #{second} (Bo) {ready} 5 min"),
        ],
        "the new one first"
    );
    let older = format!("- #{first} (Ann) {ready} 3 h ");
    assert!(lines[2].starts_with(&older), "then the first: {told:?}");
    assert!(needs(&conn));
    // That telling was of both.
    writes(&conn, "asked again");
    now += 300 * S;
    for seconds in [100, 250, 400] {
        look(&mut conn, now + seconds * S);
    }
    assert_eq!(said(&conn).len(), 2, "told of both");

    // A failure, in a chat of its own.
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = give(&mut conn, "ann");
    fail(&mut conn, task);
    let t0 = gathr::ids::now_us();
    for seconds in [0, 150, 300] {
        look(&mut conn, t0 + seconds * S);
    }
    assert_eq!(said(&conn).len(), 1, "told of the failure");
    // Its giver hears of it: the line's words change, and what it is of doesn't.
    conn.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [task])
        .unwrap();
    writes(&conn, "asked");
    for seconds in [400, 550, 700, 850] {
        look(&mut conn, t0 + seconds * S);
    }
    assert!(stuck(&conn));
    assert_eq!(said(&conn).len(), 1, "the same failure, heard");

    tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    look(&mut conn, t0 + 860 * S);
    assert!(!stuck(&conn), "restarted: its turn is queued");
    fail(&mut conn, task);
    for seconds in [870, 1020, 1169] {
        look(&mut conn, t0 + seconds * S);
        assert_eq!(said(&conn).len(), 1, "at {seconds} s");
    }
    look(&mut conn, t0 + 1170 * S);
    assert_eq!(said(&conn).len(), 2, "failed again: told again");
}

/// Scenario 6h: the lead is woken again only for what is new, and never while its last wake is
/// still to run.
#[test]
fn the_lead_is_woken_again_only_for_what_is_new() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let live = service();
    let first = idle(&mut conn, "ann");
    let t0 = gathr::ids::now_us();
    let woke = check(&mut conn, &live, t0);
    assert_eq!(woke.len(), 1, "woken for the first");
    end(&conn, &woke[0].turn);

    writes(&conn, "asked");
    for seconds in 1..=5 {
        let again = check(&mut conn, &live, t0 + seconds * S);
        assert!(again.is_empty(), "the same task: no news");
    }
    assert!(stuck(&conn));
    let ended = stretches(&conn);
    assert_eq!(ended, [(S, "wrote".to_string())], "their message was seen");

    let second = idle(&mut conn, "bo");
    let woke = check(&mut conn, &live, t0 + 6 * S);
    assert_eq!(woke.len(), 1, "a second task nobody is on is news");
    let ready = "is ready, and no turn is on it, for under a minute";
    let wake = format!(
        "Not moving: #{second} (Bo) and 1 more. Nothing waits on the person.\n\
         - #{second} (Bo) {ready}\n- #{first} (Ann) {ready}\n\
         Move it on: give, restart or cancel work, or ask the person. If nothing can be done, say why."
    );
    assert_eq!(woke[0].text, wake, "the new one first");

    // That wake is queued, then runs: a third thing waits for it to end.
    let wake = woke[0].turn.clone();
    let third = idle(&mut conn, "cy");
    let queued = check(&mut conn, &live, t0 + 7 * S);
    assert!(queued.is_empty(), "its wake is queued");
    store::set_turn(&conn, &wake, "running", None).unwrap();
    let running = check(&mut conn, &live, t0 + 8 * S);
    assert!(running.is_empty(), "its wake runs");
    end(&conn, &wake);
    let woke = check(&mut conn, &live, t0 + 9 * S);
    assert_eq!(woke.len(), 1, "its wake is over");
    let names = format!("Not moving: #{third} (Cy) and 2 more.");
    assert!(woke[0].text.starts_with(&names), "{}", woke[0].text);
    end(&conn, &woke[0].turn);

    // Trouble its giver hasn't heard only follows what nobody is on, and is still named first.
    let failed = give(&mut conn, "ann");
    fail(&mut conn, failed);
    let woke = check(&mut conn, &live, t0 + 10 * S);
    assert_eq!(woke.len(), 1, "unheard trouble is news");
    let names = format!(
        "Not moving: #{failed} (Ann) and 3 more. Nothing waits on the person.\n\
         - #{failed} (Ann) is failed: boom. Lead hears of it next, for under a minute\n"
    );
    assert!(woke[0].text.starts_with(&names), "{}", woke[0].text);
}

/// Bo's review of #157, recorded done at a commit, and Ann's work held only on that PR's merge.
fn reviewed_pr(conn: &mut Connection) -> landings::Review {
    let review = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: None,
        parent: None,
        kind: Kind::Review,
        what: "Review #157",
        why: None,
        after: &[],
        pr: Some(157),
        in_passing: false,
    };
    let review = tasks::give(conn, &review, T).unwrap().0.number;
    let ran = begin(conn, review);
    let done = Said {
        commit: Some("0123456"),
        ..Said::default()
    };
    tasks::finish(conn, "bo", review, Finish::Done, &done, T).unwrap();
    end(conn, &ran);
    tasks::ended(conn, review, Outcome::Ok, T).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks (chat_id, giver, owner, kind, what, state, reason, pr, created_ts, updated_ts)
         VALUES ('c', 'lead', 'ann', 'work', 'Build it', 'held', 'done, waiting on #157 to merge', 157, 5, 5);
         UPDATE turns SET status = 'done' WHERE status = 'queued';",
    )
    .unwrap();
    landings::review_of(conn, "c", 157).unwrap().unwrap()
}

/// Scenario 6c: a PR Gathr can't land is left to its reviewer and the reviewer is woken in one
/// write, so no check falls between the two and wakes the lead for a PR someone is about to judge.
#[test]
fn a_pr_left_to_its_reviewer_doesnt_wake_the_lead() {
    // Left to its reviewer with nobody woken, the PR is news for the lead.
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewed_pr(&mut conn);
    let left = "left to its reviewer: CI failed";
    landings::looked(&conn, ("c", 157, "0123456"), left, false).unwrap();
    let woke = check(&mut conn, &service(), gathr::ids::now_us());
    assert_eq!(woke.len(), 1, "nobody is on the PR");
    assert_eq!(woke[0].agent, "lead");

    let dir = TempDir::new();
    let mut conn = team(&dir);
    let review = reviewed_pr(&mut conn);
    let wake = "Gathr can't land #157 at 0123456: CI failed.";
    let told = landings::tell(&mut conn, &review, "0123456", "CI failed", Some(wake), T).unwrap();
    assert_eq!(told.map(|start| start.agent), Some("bo".to_string()));
    let look = landings::last_look(&conn, "c", 157).unwrap();
    let look = look.map(|(saw, moving, _)| (saw, moving));
    assert_eq!(look, Some((left.to_string(), false)), "said with the wake");
    let woke = check(&mut conn, &service(), gathr::ids::now_us());
    assert!(woke.is_empty(), "its reviewer's turn is queued");
    assert!(!stuck(&conn));
}

/// Scenario 6d: a silent turn's lead is told once, on no task, and can write to that agent. A
/// lead's own silent turn tells nobody: the stuck rule covers it, and tells the person.
#[test]
fn a_silent_turns_lead_is_told_once_and_a_leads_own_tells_nobody() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = give(&mut conn, "ann");
    let running = begin(&mut conn, task);
    let told = quiet::silent_turn(&mut conn, "c", "ann", Some(task), 720_000, "silent-trace");
    let told = told.unwrap();
    assert_eq!(told.len(), 1);
    assert_eq!((told[0].agent.as_str(), told[0].task), ("lead", None));
    assert_eq!(
        told[0].text,
        format!(
            "Ann's turn on #{task} has made no progress for 12 min. It is still running. \
             A message to them reaches that turn."
        )
    );
    let own = quiet::silent_turn(&mut conn, "c", "lead", None, 720_000, T).unwrap();
    assert!(own.is_empty(), "the lead's own turn");
    let alone = quiet::silent_turn(&mut conn, "solo", "cy", None, 720_000, T).unwrap();
    assert!(alone.is_empty(), "a chat with no Supervisor");
    // Found beside the turn's own events, by its reference.
    let recorded: Vec<Option<String>> = {
        let sql = "SELECT trace_id FROM events WHERE name = 'gathr.turn.silent_told'";
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(recorded, [Some("silent-trace".to_string())]);
    let attrs = &events(&conn, "gathr.turn.silent_told")[0].1;
    assert_eq!(
        attrs,
        &json!({ "gen_ai.agent.id": "ann", "lead": "lead", "quiet_ms": 720_000 })
    );
    end(&conn, &told[0].turn);

    // The rule wakes nobody for a member's silent turn: its lead has that telling. Other work
    // nobody is on is still news.
    let t0 = gathr::ids::now_us();
    let live = silent(&running, t0);
    assert!(check(&mut conn, &live, t0).is_empty());
    assert!(stuck(&conn), "a silent turn moves nothing");
    idle(&mut conn, "bo");
    assert_eq!(check(&mut conn, &live, t0 + S).len(), 1);

    // The lead's own silent turn: a wake would queue behind it, so the person is told.
    let dir = TempDir::new();
    let mut conn = team(&dir);
    idle(&mut conn, "ann");
    turn(&conn, "own", "lead", "running");
    let t0 = gathr::ids::now_us();
    let live = silent("own", t0);
    for seconds in [0, 150, 300] {
        let woke = check(&mut conn, &live, t0 + seconds * S);
        assert!(woke.is_empty(), "the lead's own turn is the silent one");
    }
    assert_eq!(said(&conn).len(), 1, "told at 5 minutes");
    end(&conn, "own");
    let woke = check(&mut conn, &service(), t0 + 310 * S);
    assert_eq!(woke.len(), 1, "its turn over, the lead is woken");
}
