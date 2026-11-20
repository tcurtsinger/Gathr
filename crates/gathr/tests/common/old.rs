//! An older database for the upgrade tests: a current one taken back to an earlier schema, so opening
//! it again runs the migrations after that one.

use rusqlite::Connection;

// A new migration adds its undo here, at the top.
const UNDO: &[(i64, &str)] = &[
    (66, "ALTER TABLE agents DROP COLUMN picture;"),
    (65, "ALTER TABLE chats DROP COLUMN room;"),
    (64, "DROP TABLE board_lines;"),
    (
        63,
        "ALTER TABLE task_sessions DROP COLUMN summed; ALTER TABLE engine_sessions DROP COLUMN summed;",
    ),
    (
        62,
        "DROP TABLE turn_answers; ALTER TABLE tasks DROP COLUMN settled_by;
         ALTER TABLE turns DROP COLUMN session_id;",
    ),
    (
        61,
        "ALTER TABLE turns DROP COLUMN failed_ci; ALTER TABLE waits DROP COLUMN failed_at;",
    ),
    (60, "DROP TABLE open_checks;"),
    (59, "ALTER TABLE turns DROP COLUMN placed;"),
    (58, ""),
    (57, "DROP TABLE reads;"),
    (
        56,
        "ALTER TABLE messages DROP COLUMN reply_to; ALTER TABLE messages DROP COLUMN reply_task;",
    ),
    (
        55,
        "DROP INDEX messages_opening; DROP INDEX tasks_heard; DROP INDEX tasks_by_pr;
         ALTER TABLE turns DROP COLUMN result_of; ALTER TABLE landings DROP COLUMN turn;
         ALTER TABLE messages DROP COLUMN about;",
    ),
    (54, "ALTER TABLE messages DROP COLUMN picks;"),
    (53, ""),
    (
        52,
        "ALTER TABLE agents DROP COLUMN face_seed; ALTER TABLE agents DROP COLUMN gender;
         ALTER TABLE agents DROP COLUMN skin; ALTER TABLE agents DROP COLUMN background;",
    ),
    // 0051, 0029 and 0028 rebuild a table from the columns it had, so running them again is their undo.
    (51, ""),
    (
        50,
        "ALTER TABLE changes DROP COLUMN size; ALTER TABLE changes DROP COLUMN line_of;",
    ),
    (49, "DROP TABLE held_turns;"),
    (
        48,
        "ALTER TABLE tasks DROP COLUMN look_into; ALTER TABLE asks DROP COLUMN held_ts;",
    ),
    (
        47,
        "DROP INDEX tasks_checking; ALTER TABLE tasks DROP COLUMN checks;
         DROP TABLE plan_parts; DROP TABLE plan_versions; DROP TABLE plans;",
    ),
    (
        46,
        "DROP INDEX turns_by_ask; DROP INDEX tasks_by_ask; ALTER TABLE turns DROP COLUMN ask_id;
         ALTER TABLE tasks DROP COLUMN ask_id; DROP TABLE asks;",
    ),
    (
        45,
        "ALTER TABLE tasks DROP COLUMN done; ALTER TABLE tasks DROP COLUMN proof;
         ALTER TABLE tasks DROP COLUMN place; ALTER TABLE tasks DROP COLUMN size;
         ALTER TABLE tasks DROP COLUMN size_lines; ALTER TABLE tasks DROP COLUMN started_ts;
         ALTER TABLE tasks DROP COLUMN carry; ALTER TABLE tasks DROP COLUMN edited_ts;
         ALTER TABLE tasks DROP COLUMN edited;",
    ),
    (44, "DROP TABLE reactions;"),
    (
        43,
        "ALTER TABLE turns DROP COLUMN opened_by; ALTER TABLE turns DROP COLUMN opened_version;",
    ),
    (42, "DROP TABLE task_sessions;"),
    (
        41,
        "DROP TABLE changes; ALTER TABLE tasks DROP COLUMN first_reviewed;",
    ),
    (40, "DROP TABLE review_notes;"),
    (39, "DROP TABLE quiet; DROP INDEX messages_person;"),
    (
        38,
        "ALTER TABLE landings DROP COLUMN saw; ALTER TABLE landings DROP COLUMN moving;
         ALTER TABLE landings DROP COLUMN looked_ts;",
    ),
    (
        37,
        "ALTER TABLE tasks DROP COLUMN signed_off; DROP TABLE landings;",
    ),
    (36, "ALTER TABLE waits DROP COLUMN seen;"),
    (35, "DROP TABLE friction;"),
    (34, "ALTER TABLE tasks DROP COLUMN judged;"),
    (
        33,
        "ALTER TABLE tasks DROP COLUMN in_passing; ALTER TABLE tasks DROP COLUMN heard_as;",
    ),
    (
        32,
        "ALTER TABLE tasks DROP COLUMN finish_see; ALTER TABLE tasks DROP COLUMN finish_ts;
         ALTER TABLE tasks RENAME COLUMN asks TO idle_lefts;",
    ),
    (31, "ALTER TABLE agent_context DROP COLUMN ran;"),
    (
        30,
        "ALTER TABLE agent_context DROP COLUMN told; ALTER TABLE agent_context DROP COLUMN window_of;",
    ),
    (29, ""),
    (28, ""),
    (27, "DROP INDEX turns_live; DROP INDEX tasks_asked;"),
    (26, "ALTER TABLE tasks DROP COLUMN review_done;"),
];

/// Takes a current database back to schema `version` by undoing each newer migration, newest first.
pub fn back_to(conn: &Connection, version: i64) {
    let newest = UNDO[0].0;
    assert_eq!(
        newest,
        gathr::db::SCHEMA_VERSION,
        "add the undo of migration {} to tests/common/old.rs",
        gathr::db::SCHEMA_VERSION
    );
    assert!(
        version >= UNDO.last().unwrap().0 - 1,
        "no undo back to {version}"
    );
    for (_, sql) in UNDO.iter().filter(|(n, _)| *n > version) {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
}
