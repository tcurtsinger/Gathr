//! Search (plans/search.md): what a typed word finds, that the index follows every change to the
//! messages (and survives the update's backup copy and an upgrade), archived chats, limits, and a
//! failure traced from its reference.

mod common;

use common::{TempDir, TestService, gathr};
use gathr::store::{add_message, add_turn};
use rusqlite::Connection;
use serde_json::Value;

const T: &str = "test-trace";

/// Chats `road` ("Gravel road") and `other`, one turn each of Ann's.
fn store(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('ann', 'Ann', 'ann', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('road', 'Gravel road', 1), ('other', 'Other', 2);",
    )
    .unwrap();
    add_turn(&conn, "road", "t1", T, (Some("ann"), None, None)).unwrap();
    add_turn(&conn, "other", "t2", T, (Some("ann"), None, None)).unwrap();
    conn
}

fn say(conn: &Connection, chat: &str, role: &str, text: &str) -> String {
    let turn = if chat == "road" { "t1" } else { "t2" };
    let saved = add_message(conn, chat, turn, role, "text", text).unwrap();
    saved["id"].as_str().unwrap().to_string()
}

/// The message ids `q` finds, newest first.
fn found(conn: &Connection, q: &str) -> Vec<String> {
    let found = gathr::search::search(conn, q, true, 100).unwrap().answer;
    let hits = found["messages"].as_array().unwrap();
    hits.iter()
        .map(|h| h["message"].as_str().unwrap().to_string())
        .collect()
}

/// Every message the index holds is a text message there now, and every text message is in it.
fn index_matches_messages(conn: &Connection) {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS temp.vocab USING fts5vocab(main, message_search, 'instance')",
    )
    .unwrap();
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    let stale = count(
        "SELECT count(*) FROM (SELECT DISTINCT doc FROM temp.vocab
         EXCEPT SELECT rowid FROM messages WHERE kind = 'text')",
    );
    let missing = count(
        "SELECT count(*) FROM (SELECT rowid FROM messages WHERE kind = 'text' AND body GLOB '*[a-z]*'
         EXCEPT SELECT DISTINCT doc FROM temp.vocab)",
    );
    assert_eq!(
        (stale, missing),
        (0, 0),
        "index rows with no message, text messages not indexed"
    );
    conn.execute(
        "INSERT INTO message_search (message_search) VALUES ('integrity-check')",
        [],
    )
    .unwrap();
}

/// Percent-encodes every byte but letters and digits.
fn encode(q: &str) -> String {
    q.bytes()
        .map(|b| match b.is_ascii_alphanumeric() {
            true => (b as char).to_string(),
            false => format!("%{b:02X}"),
        })
        .collect()
}

fn api(service: &TestService, query: &str) -> Value {
    let answer = service.call("GET", &format!("/api/search?{query}"));
    assert_eq!(answer.status, 200, "{query}: {}", answer.body);
    answer.json()
}

#[test]
fn every_word_must_match_as_a_word_start() {
    let dir = TempDir::new();
    let conn = store(&dir);
    let hit = say(&conn, "road", "user", "The gravel path by the river");
    say(&conn, "road", "assistant", "Gravel only");
    assert_eq!(found(&conn, "grav riv"), std::slice::from_ref(&hit));
    assert_eq!(found(&conn, "grav zzz"), Vec::<String>::new());
    assert_eq!(
        found(&conn, "ravel"),
        Vec::<String>::new(),
        "only word starts match (D2)"
    );
}

#[test]
fn typed_syntax_is_searched_as_text() {
    let dir = TempDir::new();
    let conn = store(&dir);
    let hit = say(&conn, "road", "user", "check_file said a:b and NEAR it");
    for q in [
        r#"""#,
        r#""check"#,
        "check*",
        "-check",
        "a:b",
        "NEAR(check file)",
        "AND",
        "check AND",
        "-",
        "^",
    ] {
        let searched = gathr::search::search(&conn, q, true, 30);
        assert!(searched.is_ok(), "{q:?} failed: {:?}", searched.err());
    }
    assert_eq!(
        found(&conn, "check_fi"),
        std::slice::from_ref(&hit),
        "a joined word is a phrase prefix"
    );
    assert_eq!(found(&conn, "a:b NEAR"), std::slice::from_ref(&hit));
    assert_eq!(
        found(&conn, "check - \""),
        [hit],
        "a word with no letters drops out"
    );
}

#[test]
fn case_and_accents_fold() {
    let dir = TempDir::new();
    let conn = store(&dir);
    let hit = say(&conn, "road", "assistant", "The Café opens late");
    assert_eq!(found(&conn, "cafe"), std::slice::from_ref(&hit));
    assert_eq!(found(&conn, "CAFÉ"), [hit]);
}

#[test]
fn tool_rows_are_never_hits() {
    let dir = TempDir::new();
    let conn = store(&dir);
    add_message(
        &conn,
        "road",
        "t1",
        "assistant",
        "tool",
        r#"{"tool":"Write","status":"ok"}"#,
    )
    .unwrap();
    assert_eq!(found(&conn, "write"), Vec::<String>::new());
    assert_eq!(found(&conn, "tool"), Vec::<String>::new());
}

#[test]
fn the_index_follows_every_change_and_the_backup_copy() {
    let dir = TempDir::new();
    let mut conn = store(&dir);
    conn.execute(
        "INSERT INTO chats (id, title, created_ts) VALUES ('gone', 'Gone', 3)",
        [],
    )
    .unwrap();
    add_turn(&conn, "gone", "t3", T, (Some("ann"), None, None)).unwrap();
    let first = say(&conn, "road", "user", "gravel first");
    for n in 0..5 {
        add_message(
            &conn,
            "gone",
            "t3",
            "user",
            "text",
            &format!("gravel gone {n}"),
        )
        .unwrap();
    }
    let edited = say(&conn, "other", "assistant", "draft words");
    let last = say(&conn, "road", "user", "gravel last");
    gathr::store::set_message_body(&conn, &edited, "final gravel words").unwrap();
    assert_eq!(
        found(&conn, "draft"),
        Vec::<String>::new(),
        "the old words are gone"
    );
    assert_eq!(found(&conn, "final grav"), std::slice::from_ref(&edited));

    gathr::archive::archive(&mut conn, "gone", T).unwrap();
    gathr::archive::delete(&mut conn, "gone", T).unwrap();
    assert_eq!(
        found(&conn, "gravel"),
        [last.clone(), edited.clone(), first.clone()]
    );
    index_matches_messages(&conn);

    // The update's backup (app::keep_db) is VACUUM INTO: the copy's rowids must stay the
    // index's, with the deleted chat's gap before them.
    let copy = dir.0.join("kept.db");
    conn.execute("VACUUM INTO ?1", [copy.to_string_lossy()])
        .unwrap();
    let kept = Connection::open(&copy).unwrap();
    index_matches_messages(&kept);
    assert_eq!(found(&kept, "gravel"), [last, edited, first]);
}

#[test]
fn archived_chats_are_in_by_default_and_marked() {
    let dir = TempDir::new();
    let mut conn = store(&dir);
    let archived = say(&conn, "road", "user", "gravel in an archived chat");
    let open = say(&conn, "other", "assistant", "gravel 😀 漢 in an open one");
    gathr::archive::archive(&mut conn, "road", T).unwrap();
    drop(conn);
    let service = TestService::start(&dir.0);

    let all = api(&service, "q=grav");
    let hits = all["messages"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "{all}");
    assert_eq!(
        (hits[0]["message"].as_str(), hits[0]["archived"].is_null()),
        (Some(&*open), true)
    );
    assert_eq!(hits[1]["message"].as_str(), Some(&*archived));
    assert!(hits[1]["archived"].is_string(), "{all}");
    assert_eq!(
        (hits[0]["agent"].as_str(), hits[0]["chat_title"].as_str()),
        (Some("ann"), Some("Other"))
    );
    assert_eq!(all["chats"][0]["chat"], "road", "the title matches: {all}");
    assert!(all["chats"][0]["archived"].is_string(), "{all}");

    // Marks are UTF-16 offsets into the snippet, past the emoji and the CJK character.
    let snippet: Vec<u16> = hits[0]["snippet"]
        .as_str()
        .unwrap()
        .encode_utf16()
        .collect();
    let mark = &hits[0]["marks"][0];
    let (start, end) = (
        mark[0].as_u64().unwrap() as usize,
        mark[1].as_u64().unwrap() as usize,
    );
    assert_eq!(
        String::from_utf16(&snippet[start..end]).unwrap(),
        "gravel",
        "{}",
        hits[0]
    );

    let open_only = api(&service, "q=grav&archived=false");
    assert_eq!(
        open_only["messages"].as_array().unwrap().len(),
        1,
        "{open_only}"
    );
    assert_eq!(open_only["chats"], serde_json::json!([]), "{open_only}");
    service.stop();
}

#[test]
fn a_folder_from_before_search_is_found_after_the_upgrade() {
    let dir = TempDir::new();
    let conn = store(&dir);
    let old = say(&conn, "road", "user", "gravel from before");
    say(&conn, "road", "assistant", "tool-free reply");
    // The schema as 0014 left it: no index, no triggers, and none of the later migrations' columns.
    conn.execute_batch(
        "DROP TRIGGER message_search_insert; DROP TRIGGER message_search_delete;
         DROP TRIGGER message_search_update; DROP TABLE message_search;
         DROP TABLE project_checks; DROP TABLE project_check_allowed; ALTER TABLE projects DROP COLUMN ci;
         ALTER TABLE projects DROP COLUMN merge_method;
         ALTER TABLE projects DROP COLUMN ci_source; ALTER TABLE projects DROP COLUMN ci_set_ts;
         ALTER TABLE projects DROP COLUMN merge_source; ALTER TABLE projects DROP COLUMN merge_set_ts; ALTER TABLE tasks DROP COLUMN heard_in;
         ALTER TABLE tasks DROP COLUMN pr_head;
         DROP TABLE handoffs; DROP TABLE check_steps; DROP TABLE check_runs; DROP TABLE check_caches;
         DROP TABLE pr_cleanup;
         PRAGMA user_version = 14;",
    )
    .unwrap();
    let newer = say(&conn, "road", "user", "gravel saved before the upgrade too");
    // Last, as saving a message reads the column.
    conn.execute_batch(
        "ALTER TABLE messages DROP COLUMN to_agent; ALTER TABLE messages DROP COLUMN copy_of;",
    )
    .unwrap();
    drop(conn);
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    assert_eq!(found(&conn, "gravel"), [newer, old]);
    index_matches_messages(&conn);
}

#[test]
fn limits_hold_and_an_empty_query_finds_nothing() {
    let dir = TempDir::new();
    let conn = store(&dir);
    for n in 0..120 {
        say(&conn, "road", "user", &format!("gravel {n}"));
    }
    drop(conn);
    let service = TestService::start(&dir.0);
    let count = |q: &str| api(&service, q)["messages"].as_array().unwrap().len();
    assert_eq!(count("q=gravel"), 30, "the default");
    assert_eq!(count("q=gravel&limit=500"), 100, "the most");
    let long = format!("gravel{}zzz", " ".repeat(250));
    assert_eq!(
        count(&format!("q={}", encode(&long))),
        30,
        "words past 200 characters are cut"
    );
    let empty = api(&service, "q=%20%20");
    assert_eq!(empty, serde_json::json!({ "chats": [], "messages": [] }));
    service.stop();
}

#[test]
fn a_failed_search_traces_from_its_reference() {
    let dir = TempDir::new();
    let conn = store(&dir);
    say(&conn, "road", "user", "gravel");
    let service = TestService::start(&dir.0);
    conn.execute_batch("DROP TABLE message_search").unwrap();
    let failed = service.call("GET", "/api/search?q=gravel");
    assert_eq!(failed.status, 500, "{}", failed.body);
    let failed = failed.json();
    assert_eq!(failed["error"], "Search failed", "{failed}");
    let reference = failed["ref"].as_str().unwrap();
    service.stop();

    let (ok, trace) = gathr(&dir.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let events = trace["events"].as_array().unwrap();
    let search = events.iter().find(|e| e["name"] == "gathr.search.query");
    let search = search.unwrap_or_else(|| panic!("no search event: {trace}"));
    assert_eq!(search["status"], "error", "{trace}");
    assert!(
        search["attrs"]["reason"]
            .as_str()
            .unwrap()
            .contains("message_search"),
        "{trace}"
    );
    assert!(
        !trace.to_string().contains("gravel"),
        "the query's words are never recorded: {trace}"
    );
}
