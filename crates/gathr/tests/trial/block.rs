//! One block (R5, steps 2 to 6): the service started with the block's arm, each case run watched to
//! its end or its limits, an approval answered no and its run spoiled, every run scored before its
//! chat is archived, and the service stopped.

use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

use super::cases::{Arm, Block, Case, GIVER, Seat};
use super::common::TestService;
use super::setup::{Ids, say};
use super::{file, project, score};

/// A case run under way.
struct Run<'a> {
    seat: &'a Seat,
    agent: String,
    chat: String,
    task: Option<i64>,
    message: Option<String>,
    /// How it ended, once it has: by its case (`None`) or by a limit, an approval or the account.
    ended: Option<Option<&'static str>>,
    approvals: u64,
    seconds: u64,
}

/// What a block gave: its runs' rows, and how the account ended it, if it did.
pub struct Done {
    pub rows: Vec<Value>,
    pub account: Option<&'static str>,
}

/// The run's chat's turns, but Ana's (held by her pause): agent, status and what holds it.
fn turns(conn: &Connection, chat: &str, ana: &str) -> Vec<(String, String, Option<String>)> {
    let sql = "SELECT agent_id, status, held FROM turns WHERE chat_id = ?1 AND agent_id IS NOT ?2 ORDER BY created_ts";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([chat, ana], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)));
    rows.unwrap().collect::<Result<_, _>>().unwrap()
}

fn one(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> i64 {
    conn.query_row(sql, args, |r| r.get(0)).unwrap()
}

/// Input, cache read, cache write and output tokens from the chat's turn ends (usage is recorded
/// nowhere earlier).
fn usage(conn: &Connection, chat: &str) -> [u64; 4] {
    let keys = [
        "input_tokens",
        "cache_read.input_tokens",
        "cache_creation.input_tokens",
        "output_tokens",
    ];
    keys.map(|k| {
        let sql = format!(
            "SELECT COALESCE(sum(json_extract(attrs, '$.\"gen_ai.usage.{k}\"')), 0) FROM event_log
             WHERE conversation_id = ?1 AND name LIKE 'invoke_agent %' AND phase = 'end'"
        );
        one(conn, &sql, [chat]) as u64
    })
}

/// Why the run should end now, if it should: `Some(None)` when its case is over.
fn ending(
    conn: &Connection,
    run: &Run,
    ana: &str,
    standin: bool,
    began: Instant,
) -> Option<Option<&'static str>> {
    let turns = turns(conn, &run.chat, ana);
    if turns.iter().any(|t| t.2.as_deref() == Some("usage")) {
        return Some(Some("usage_limit"));
    }
    if turns.iter().any(|t| t.1 == "waiting_sign_in") {
        return Some(Some("signed_out"));
    }
    let [input, read, write, _] = usage(conn, &run.chat);
    let (tokens, secs) = run.seat.case.limits();
    // The stand-in's held engines show a time limit without waiting minutes for it.
    let secs = if standin { secs.min(20) } else { secs };
    if input + read + write > tokens {
        return Some(Some("over_cap"));
    }
    if began.elapsed().as_secs() > secs {
        return Some(Some("timed_out"));
    }
    let open = |t: &&(String, String, Option<String>)| {
        ["queued", "running", "waiting_sign_in"].contains(&t.1.as_str())
    };
    let settled = !turns.is_empty() && !turns.iter().any(|t| open(&t));
    settled.then_some(None)
}

fn post(service: &TestService, path: &str, body: Value) {
    let answered = service.send("POST", path, body);
    assert_eq!(answered.status, 200, "{path}: {}", answered.body);
}

/// Runs the written block on `service`, started for it, and stops it: each run's row.
pub fn run(
    service: TestService,
    data: &Path,
    ids: &Ids,
    block: &Block,
    written: &[Value],
    standin: bool,
) -> Done {
    let ana = ids.agent(GIVER).to_string();
    let mut runs: Vec<Run> = block
        .seats
        .iter()
        .zip(written)
        .map(|(seat, w)| Run {
            seat,
            agent: ids.agent(seat.agent).to_string(),
            chat: w["chat"].as_str().unwrap().to_string(),
            task: w["task"].as_i64(),
            message: w["message"].as_str().map(String::from),
            ended: None,
            approvals: 0,
            seconds: 0,
        })
        .collect();
    let (conn, _) = gathr::db::open_read_only(&gathr::data::DataDir::new(data).db()).unwrap();
    let began = Instant::now();
    while runs.iter().any(|r| r.ended.is_none()) {
        std::thread::sleep(Duration::from_secs(2));
        for run in runs.iter_mut().filter(|r| r.ended.is_none()) {
            let history = service
                .call("GET", &format!("/api/chats/{}", run.chat))
                .json();
            for asked in history["approvals"].as_array().into_iter().flatten() {
                let request = asked["request_id"].as_str().unwrap_or_default();
                post(
                    &service,
                    &format!("/api/chats/{}/approvals/{request}", run.chat),
                    json!({ "allow": false }),
                );
                run.approvals += 1;
            }
            let why = match run.approvals {
                0 => ending(&conn, run, &ana, standin, began),
                _ => Some(Some("spoiled")),
            };
            let Some(why) = why else { continue };
            run.ended = Some(why);
            run.seconds = began.elapsed().as_secs();
            say(
                data,
                &format!(
                    "block {}: {} on {} ended {}",
                    block.n,
                    run.seat.case.name(),
                    run.seat.engine.name(),
                    why.unwrap_or("by its case")
                ),
            );
            if why.is_some() {
                post(
                    &service,
                    &format!("/api/chats/{}/stop", run.chat),
                    json!({}),
                );
            }
        }
    }
    // A stopped turn ends as its engine stops it; one that doesn't is scored as it stands.
    let running = "SELECT count(*) FROM turns WHERE chat_id = ?1 AND status = 'running'";
    let deadline = Instant::now() + Duration::from_secs(60);
    while runs.iter().any(|r| one(&conn, running, [&r.chat]) > 0) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));
    }
    let rows = runs.iter().map(|r| row(&conn, block, r, &ana)).collect();
    for run in &runs {
        post(
            &service,
            &format!("/api/chats/{}/archive", run.chat),
            json!({}),
        );
    }
    let account = runs.iter().find_map(|r| {
        r.ended
            .flatten()
            .filter(|e| ["usage_limit", "signed_out"].contains(e))
    });
    service.stop();
    Done { rows, account }
}

/// Stops the trial unless the run's records show its arm: on each turn start of its agent, role
/// lines' version with the lines and none without.
fn check_arm(conn: &Connection, block: &Block, run: &Run) {
    let with = block.arm == Arm::With;
    let wrong_starts = "SELECT count(*) FROM event_log WHERE conversation_id = ?1
        AND name LIKE 'invoke_agent %' AND phase = 'start' AND json_extract(attrs, '$.\"gen_ai.agent.id\"') = ?2
        AND (json_extract(attrs, '$.\"gathr.role.version\"') IS NULL) = ?3";
    let wrong = one(conn, wrong_starts, params![run.chat, run.agent, with]) > 0;
    assert!(
        !wrong,
        "block {}, {} on {}: the records don't show the {} arm. The trial stops here.",
        block.n,
        run.seat.case.name(),
        run.seat.engine.name(),
        block.arm.name()
    );
}

/// Scores the run before its chat is archived (R5, step 5).
fn row(conn: &Connection, block: &Block, run: &Run, ana: &str) -> Value {
    check_arm(conn, block, run);
    let chat = run.chat.as_str();
    let [input, read, write, output] = usage(conn, chat);
    let ended = "SELECT count(*) FROM turns WHERE chat_id = ?1 AND agent_id IS NOT ?2 AND status IN ('done', 'stopped', 'failed')";
    let tools = "SELECT count(*) FROM messages WHERE chat_id = ?1 AND kind = 'tool'";
    let (mut counts, case) = match run.seat.case {
        Case::T1 => t1(conn, run),
        Case::T8 => t8(conn, run),
    };
    let common = json!({ "input_tokens": input, "cache_read": read, "cache_write": write, "output_tokens": output,
        "turns": one(conn, ended, [chat, ana]), "tool_calls": one(conn, tools, [chat]), "approvals": run.approvals,
        "seconds": run.seconds });
    counts.extend(common.as_object().cloned().unwrap_or_default());
    let outcome = run.ended.flatten().unwrap_or(case);
    file::run_row(block, run.seat, outcome, counts)
}

fn t1(conn: &Connection, run: &Run) -> (Map<String, Value>, &'static str) {
    let sql =
        "SELECT path FROM agent_folders WHERE chat_id = ?1 AND agent_id = ?2 AND kind = 'worktree'
               ORDER BY made_ts DESC LIMIT 1";
    let worktree: Option<String> = conn
        .query_row(sql, [&run.chat, &run.agent], |r| r.get(0))
        .optional()
        .unwrap();
    let task = run.task.unwrap();
    let base = project::sources(&project::fixture().join("project"));
    let (diff, after, hidden) = match worktree
        .map(std::path::PathBuf::from)
        .filter(|w| w.join(".git").exists())
    {
        Some(w) => (
            project::diff(&w),
            project::sources(&w),
            project::hidden_test(&w),
        ),
        None => (String::new(), Default::default(), false),
    };
    let built = score::built(&diff, [&base[0], &base[1]], [&after[0], &after[1]]);
    let done = "SELECT count(*) FROM tasks WHERE number = ?1 AND state = 'done'";
    score::t1(&built, hidden, one(conn, done, [task]) > 0)
}

fn t8(conn: &Connection, run: &Run) -> (Map<String, Value>, &'static str) {
    let message = run.message.as_deref().unwrap_or_default();
    let reacted = "SELECT count(*) FROM reactions WHERE message_id = ?1 AND agent_id = ?2";
    let sent =
        "SELECT count(*) FROM event_log WHERE name = 'gathr.message.send' AND conversation_id = ?1
                AND json_extract(attrs, '$.from') = ?2";
    let replied = "SELECT COALESCE(sum(length(m.body)), 0) FROM messages m JOIN turns t ON t.id = m.turn_id
                   WHERE m.chat_id = ?1 AND t.agent_id = ?2 AND m.role = 'assistant' AND m.kind = 'text'
                   AND m.to_agent IS NULL";
    let (who, chat) = (run.agent.as_str(), run.chat.as_str());
    score::t8(
        one(conn, reacted, [message, who]) > 0,
        one(conn, sent, [chat, who]) as u64,
        one(conn, replied, [chat, who]) as u64,
    )
}
