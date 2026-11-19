use rusqlite::params;

/// A team chat of `rounds` rounds: the person's message, the lead's three tasks each with a turn of
/// nine messages, a done line for each, and a hear-back. 32 messages a round.
fn team_chat(rounds: i64) -> (rusqlite::Connection, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("gathr-topics-{}.db", crate::ids::uuid()));
    let conn = crate::db::open(&path).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('acc', 'claude', 'A', 'a', 0);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('lead', 'Lead', 'lead', 'acc', 0),
             ('ann', 'Ann', 'ann', 'acc', 0);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 0);
         INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES ('c', 'lead', 'supervisor', 0),
             ('c', 'ann', 'member', 0);",
    )
    .unwrap();
    let mut ts = 0;
    let mut message = |turn: Option<&str>, role: &str, about: Option<String>| {
        ts += 1;
        conn.execute(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, about)
             VALUES (?1, 'c', ?2, ?3, ?4, 'text', '', ?5)",
            params![format!("m{ts}"), turn, ts, role, about],
        )
        .unwrap();
        ts
    };
    // Each turn says what opened it, as every turn the service starts now does.
    let turn = |id: &str,
                (agent, task, answers): (&str, Option<i64>, Option<&str>),
                opened: &str| {
        conn.execute(
            "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, task, answers, opened_by)
             VALUES (?1, 'c', 'x', 'done', 0, ?2, ?3, ?4, ?5)",
            params![id, agent, task, answers, opened],
        )
        .unwrap();
    };
    for round in 0..rounds {
        let asked = format!("asked{round}");
        turn(&asked, ("lead", None, None), "person");
        message(Some(&asked), "user", None);
        for k in 1..=3 {
            let n = round * 3 + k;
            conn.execute(
                "INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts,
                                    updated_ts)
                 VALUES (?1, 'c', 'lead', 'ann', ?2, 'work', 'w', 'done', 0, 0)",
                params![n, asked],
            )
            .unwrap();
            let work = format!("work{n}");
            turn(&work, ("ann", Some(n), Some(&asked)), "brief");
            message(Some(&work), "gathr", None);
            for _ in 0..8 {
                message(Some(&work), "assistant", None);
            }
            message(None, "gathr", Some(format!("t:{n}")));
        }
        let back = format!("back{round}");
        turn(&back, ("lead", None, Some(&asked)), "hear_back");
        message(Some(&back), "gathr", None);
    }
    (conn, path)
}

fn remove(path: &std::path::Path) {
    for end in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", path.display()));
    }
}

/// The steps SQLite takes to read the topics of a team chat of `rounds` rounds. Every turn, task and
/// line is tied, and none is tied to nothing.
fn steps(rounds: i64) -> i64 {
    let (conn, path) = team_chat(rounds);
    let topics = super::Topics::of_chat(&conn, "c").unwrap();
    let all = topics.all();
    let tied = all.turns.iter().map(|(_, t)| t);
    let tied = tied.chain(all.tasks.iter().map(|(_, t)| t));
    assert!(
        tied.chain(all.lines.iter().map(|(_, t)| t))
            .all(|t| !t.alone)
    );
    let steps = topics.src.steps;
    drop(topics);
    drop(conn);
    remove(&path);
    steps
}

/// The live event's topic of the last round's member turn and hear-back, and the steps each took.
fn live(rounds: i64) -> Vec<(Option<String>, i64)> {
    let (conn, path) = team_chat(rounds);
    let last = rounds - 1;
    let turns = [format!("work{}", last * 3 + 3), format!("back{last}")];
    let found = turns
        .iter()
        .map(|turn| super::of_turn_counted(&conn, "c", turn).unwrap());
    let found = found.collect();
    drop(conn);
    remove(&path);
    found
}

/// A live `turn` event's topic follows that turn's own chain, never the whole chat: on the busiest
/// path, a chat ten times as long costs the same.
#[test]
fn a_live_turns_topic_costs_the_same_in_a_chat_ten_times_as_long() {
    let (small, large) = (live(6), live(60));
    let topics =
        |found: &[(Option<String>, i64)]| found.iter().map(|f| f.0.clone()).collect::<Vec<_>>();
    // The lead's hear-back on no task of its own is a main message of its own (0078).
    assert_eq!(
        topics(&small),
        [Some("t:18".into()), Some("b:back5".into())]
    );
    assert_eq!(
        topics(&large),
        [Some("t:180".into()), Some("b:back59".into())]
    );
    for (s, l) in small.iter().zip(&large) {
        assert!(
            l.1 <= s.1 + s.1 / 5,
            "{} steps in 192 messages, {} in 1,920",
            s.1,
            l.1
        );
    }
}

/// One read per table, never a query per row (0073's lesson): twice the chat is about twice the
/// steps, not four times.
#[test]
fn the_topics_cost_grows_with_the_chat_not_with_its_rows_times_the_chat() {
    let (small, large) = (steps(50), steps(100));
    assert!(
        large < small * 3,
        "{small} steps for 50 rounds, {large} for 100"
    );
}

/// 0078: the lead's turn a send-back queued is a main message of its own, as is the turn that wrote
/// the version it sent back, read whole or one row at a time; the version's check sits under that
/// turn.
#[test]
fn the_turn_a_send_back_queued_and_the_turn_that_wrote_the_version_are_each_main() {
    let path = std::env::temp_dir().join(format!("gathr-topics-{}.db", crate::ids::uuid()));
    let conn = crate::db::open(&path).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('acc', 'claude', 'A', 'a', 0);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('lead', 'Lead', 'lead', 'acc', 0);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 0);
         INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES ('c', 'lead', 'supervisor', 0);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, opened_by) VALUES
             ('asked', 'c', 'x', 'done', 1, 'lead', 'person');
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, answers, opened_by) VALUES
             ('back', 'c', 'x', 'done', 5, 'lead', 'asked', 'sent_back');
         INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
             ('ask', 'c', 'asked', 1, 'user', 'text', 'Add tags'),
             ('v1', 'c', 'asked', 2, 'assistant', 'text', 'Plan'),
             ('told', 'c', 'back', 5, 'gathr', 'text', 'The person sent back version 1');
         INSERT INTO asks (id, chat_id) VALUES ('ask', 'c');
         INSERT INTO plans (id, chat_id, ask_id, lead, created_ts) VALUES (1, 'c', 'ask', 'lead', 2);
         INSERT INTO plan_versions (id, plan_id, n, message_id, turn_id, title, text, created_ts, decided, decided_line)
             VALUES (1, 1, 1, 'v1', 'asked', 'Tags', 'x', 2, 'sent_back', 'told');
         INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts, checks)
             VALUES (1, 'c', 'lead', 'lead', 'asked', 'review', 'Check plan v1', 'done', 2, 2, 1);",
    )
    .unwrap();
    let whole = super::Topics::of_chat(&conn, "c").unwrap();
    let rows = super::Topics::on_demand(&conn, "c");
    let found = [
        whole.turn("back"),
        rows.turn("back"),
        whole.turn("asked"),
        rows.turn("asked"),
        whole.task(1),
        rows.task(1),
    ];
    let found = found.map(|t| t.unwrap().key);
    let want = [
        "b:back", "b:back", "b:asked", "b:asked", "b:asked", "b:asked",
    ];
    assert_eq!(found, want);
    rows.src.read().unwrap();
    drop((whole, rows));
    drop(conn);
    remove(&path);
}

/// A chain that loops ends where it loops, tied to nothing, and says so.
#[test]
fn a_chain_that_loops_ties_to_nothing() {
    let path = std::env::temp_dir().join(format!("gathr-topics-{}.db", crate::ids::uuid()));
    let conn = crate::db::open(&path).unwrap();
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 0);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, answers) VALUES
             ('a', 'c', 'x', 'done', 1, 'b'), ('b', 'c', 'x', 'done', 2, 'a');",
    )
    .unwrap();
    let topics = super::Topics::of_chat(&conn, "c").unwrap();
    let (a, b) = (topics.turn("a").unwrap(), topics.turn("b").unwrap());
    assert_eq!((a.key.as_str(), b.key.as_str()), ("b:b", "b:b"));
    assert_eq!(b.why, "tied to nothing: a chain that loops at answers a");
    assert!(b.broken && !a.alone, "{a:?} {b:?}");
    drop(topics);
    drop(conn);
    for end in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", path.display()));
    }
}
