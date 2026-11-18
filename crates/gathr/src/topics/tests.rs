use rusqlite::params;

/// The steps SQLite takes to read the topics of a team chat of `rounds` rounds: the person's
/// message, the lead's three tasks each with a turn of nine messages, a done line for each, and a
/// hear-back. Every one of them is tied, and none is tied to nothing.
fn steps(rounds: i64) -> i64 {
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
    let turn = |id: &str, (agent, task, answers): (&str, Option<i64>, Option<&str>)| {
        conn.execute(
            "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, task, answers)
             VALUES (?1, 'c', 'x', 'done', 0, ?2, ?3, ?4)",
            params![id, agent, task, answers],
        )
        .unwrap();
    };
    for round in 0..rounds {
        let asked = format!("asked{round}");
        turn(&asked, ("lead", None, None));
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
            turn(&work, ("ann", Some(n), Some(&asked)));
            message(Some(&work), "gathr", None);
            for _ in 0..8 {
                message(Some(&work), "assistant", None);
            }
            message(None, "gathr", Some(format!("t:{n}")));
        }
        let back = format!("back{round}");
        turn(&back, ("lead", None, Some(&asked)));
        message(Some(&back), "gathr", None);
    }
    let topics = super::Topics::of_chat(&conn, "c").unwrap();
    let all = topics.all();
    let tied = all.turns.iter().map(|(_, t)| t);
    let tied = tied.chain(all.tasks.iter().map(|(_, t)| t));
    assert!(
        tied.chain(all.lines.iter().map(|(_, t)| t))
            .all(|t| !t.alone)
    );
    let steps = topics.facts.steps;
    drop(topics);
    drop(conn);
    for end in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", path.display()));
    }
    steps
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
