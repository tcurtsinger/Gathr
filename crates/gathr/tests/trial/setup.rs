//! The trial's setup (R5): its accounts, agents and project once, through the API; then each block's
//! chats, tasks and message through the library with the service stopped, as `gathr seed` writes,
//! with Ana, a paused lead, as the giver and sender.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use gathr::tasks::{Brief, Give, Kind};
use gathr::team::Role;
use gathr::words::Opened;
use rusqlite::Connection;
use serde_json::{Value, json};

use super::cases::{Arm, Block, Engine, GIVER, SEATS};
use super::common::{TestService, sign_in_account};
use super::project;

/// Where the trial keeps its own files, in its data folder: the project, the log, each block's
/// writes and the results.
pub fn dir(data: &Path) -> PathBuf {
    data.join("trial")
}

/// Says `text` on the trial's output and in `trial/log.txt`.
pub fn say(data: &Path, text: &str) {
    use std::io::Write;
    println!("{text}");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir(data).join("log.txt"));
    if let Ok(mut log) = log {
        let _ = writeln!(log, "{} {text}", gathr::ids::utc(gathr::ids::now_us()));
    }
}

/// Archives each chat of the "Trial" project a killed trial left in the list, with the service
/// stopped: its queued turns would otherwise start at the next start, with no one watching or
/// counting them. How many it archived. A fresh folder has none.
pub fn clean(data: &Path) -> usize {
    let folder = gathr::data::DataDir::new(data);
    if !folder.db().is_file() {
        return 0;
    }
    let _lock = folder
        .take_lock()
        .expect("the service is stopped before the trial starts");
    let mut conn = gathr::db::open(&folder.db()).unwrap();
    let left: Vec<String> = {
        let sql = "SELECT c.id FROM chats c JOIN projects p ON p.id = c.project_id
                   WHERE p.name = 'Trial' AND c.archived_ts IS NULL";
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    };
    let trace = gathr::ids::trace_id();
    for chat in &left {
        gathr::archive::archive(&mut conn, chat, &trace).unwrap();
    }
    left.len()
}

/// Whether two paths name one folder: canonical where they exist, and on Windows in any case.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    let seen = |p: &Path| {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let p = p.to_string_lossy().to_string();
        if cfg!(windows) { p.to_lowercase() } else { p }
    };
    seen(a) == seen(b)
}

/// The ids the trial made once: each engine's account and each agent, by name, and the project.
pub struct Ids {
    pub accounts: BTreeMap<&'static str, String>,
    pub agents: BTreeMap<String, String>,
    pub project: String,
    pub folder: PathBuf,
}

impl Ids {
    pub fn agent(&self, name: &str) -> &str {
        &self.agents[name]
    }
}

fn listed(service: &TestService, path: &str, key: &str) -> Vec<Value> {
    let found = service.call("GET", path);
    assert_eq!(found.status, 200, "{path}: {}", found.body);
    found.json()[key].as_array().cloned().unwrap_or_default()
}

/// Each engine's account. On the stand-in, made with a made-up login; on real engines, the one the
/// person signed in to in this folder.
fn accounts(service: &TestService, data: &Path, standin: bool) -> BTreeMap<&'static str, String> {
    let mut found = BTreeMap::new();
    for engine in [Engine::Claude, Engine::Codex].map(Engine::name) {
        let have = listed(service, "/api/accounts", "accounts");
        let id = match have.iter().find(|a| a["engine"] == engine) {
            Some(a) => a["id"].as_str().unwrap().to_string(),
            None if standin => {
                let body = json!({ "engine": engine, "name": engine });
                let added = service.send("POST", "/api/accounts", body);
                assert_eq!(added.status, 200, "{}", added.body);
                let id = added.json()["id"].as_str().unwrap().to_string();
                match engine {
                    "codex" => {
                        let folder = data.join("accounts").join(&id).join("codex");
                        std::fs::create_dir_all(&folder).unwrap();
                        std::fs::write(
                            folder.join("auth.json"),
                            r#"{"tokens":{"refresh_token":"made-up"}}"#,
                        )
                        .unwrap();
                    }
                    _ => sign_in_account(data, &id),
                }
                id
            }
            None => panic!("no {engine} account in this folder: sign in to one in Settings first"),
        };
        let account = service.call("GET", &format!("/api/accounts/{id}")).json();
        let login = account["login"].as_str().unwrap_or_default();
        assert!(
            standin || ["signed_in", "stale"].contains(&login),
            "the {engine} account is {login}: sign in again"
        );
        found.insert(engine, id);
    }
    found
}

/// The agents, made once on the room's models and efforts, approvals `auto` as the room's are.
fn agents(
    service: &TestService,
    accounts: &BTreeMap<&'static str, String>,
) -> BTreeMap<String, String> {
    let mut made = BTreeMap::new();
    let ana = (GIVER, Engine::Claude, None, None);
    let seats = SEATS
        .iter()
        .map(|s| (s.agent, s.engine, Some(s.model), Some(s.effort)));
    for (name, engine, model, effort) in std::iter::once(ana).chain(seats) {
        let have = listed(service, "/api/agents", "agents");
        let id = match have.iter().find(|a| a["name"] == name) {
            Some(a) => a["id"].as_str().unwrap().to_string(),
            None => {
                let body = json!({ "name": name, "account": accounts[engine.name()], "model": model,
                    "effort": effort, "approvals": "auto" });
                let added = service.send("POST", "/api/agents", body);
                assert_eq!(added.status, 200, "{name}: {}", added.body);
                added.json()["id"].as_str().unwrap().to_string()
            }
        };
        made.insert(name.to_string(), id);
    }
    made
}

/// The accounts, agents and project, each made the first time only.
pub fn prepare(service: &TestService, data: &Path, standin: bool) -> Ids {
    let folder = dir(data).join("project");
    if !folder.exists() {
        project::make(&folder);
    }
    let accounts = accounts(service, data, standin);
    let agents = agents(service, &accounts);
    let have = listed(service, "/api/projects", "projects");
    let project = match have.iter().find(|p| p["name"] == "Trial") {
        Some(p) => p["id"].as_str().unwrap().to_string(),
        None => {
            let members = json!([{ "agent": agents[GIVER], "role": "supervisor" }]);
            let body = json!({ "name": "Trial", "folder": folder, "members": members });
            let made = service.send("POST", "/api/projects", body);
            assert_eq!(made.status, 200, "{}", made.body);
            made.json()["id"].as_str().unwrap().to_string()
        }
    };
    Ids {
        accounts,
        agents,
        project,
        folder,
    }
}

/// The model each seat's setting resolves to, by agent, and each engine's copy's version.
pub fn models(service: &TestService, ids: &Ids) -> (BTreeMap<String, String>, Value) {
    let mut resolved = BTreeMap::new();
    let mut copies = serde_json::Map::new();
    for (engine, account) in &ids.accounts {
        let listed = service.call("GET", &format!("/api/accounts/{account}/models"));
        assert_eq!(listed.status, 200, "{engine}'s models: {}", listed.body);
        let listed = listed.json();
        let version = listed["version"]
            .as_str()
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default();
        let plain = version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
        if version.starts_with(|c: char| c.is_ascii_digit()) && plain {
            copies.insert(engine.to_string(), version.into());
        }
        for seat in SEATS.iter().filter(|s| s.engine.name() == *engine) {
            let models = listed["models"].as_array().cloned().unwrap_or_default();
            let row = models.iter().find(|m| m["id"] == seat.model);
            let to = row
                .and_then(|m| m["resolved"].as_str())
                .unwrap_or("not listed");
            resolved.insert(seat.agent.to_string(), to.to_string());
        }
    }
    (resolved, Value::Object(copies))
}

pub const T1_WHAT: &str = "Add formatDuration to the duration module";
pub const T1_WHY: &str = "Turn a number of milliseconds into the shortest text parseDuration reads back to the same \
                          number: largest unit first, no zero parts. 5400000 gives \"1h30m\", 90500 gives \"1m30s500ms\", 250 \
                          gives \"250ms\", 0 gives \"0s\". Export it, and add a command-line option so `node src/cli.js \
                          --format 5400000` prints `1h30m`.";
pub const T1_DONE: &str = "formatDuration gives those four results and `--format` prints them";
pub const T1_PROOF: &str = "`node --test` passes, with a test for the four examples";
pub const T1_WHERE: &str = "src/duration.js and src/cli.js on your branch";
pub const T2_WHAT: &str = "Review the commit on branch trial/planted";
pub const T2_WHY: &str =
    "The ask it answers: Accept upper-case units: `parseDuration(\"1H30M\")` returns NaN.";
pub const T5_ASK: &str = "Add a --json option to src/cli.js that prints the result as a JSON object instead of plain \
                          text. Two things are open: whether its keys are in camelCase or snake_case, and whether the \
                          object also carries the text it read or only the milliseconds.";
pub const T8_TEXT: &str = "Plan: you take the parser, I review after. OK?";

/// What a block needs written: its chats, each run's agent, the project and the giver.
fn asked(ids: &Ids, block: &Block) -> Value {
    let runs: Vec<Value> = block
        .seats
        .iter()
        .map(|s| json!({ "case": s.case.name(), "agent": ids.agent(s.agent) }))
        .collect();
    json!({ "project": ids.project, "folder": ids.folder, "ana": ids.agent(GIVER), "runs": runs })
}

/// Writes the block with the service stopped, in a process of the trial's own test binary that has
/// the block's arm set (R2): with `GATHR_TRIAL_WITHOUT`, a review's brief is given in its words
/// without the line. Each run's chat, and its task or message.
pub fn write_block(data: &Path, ids: &Ids, block: &Block) -> Vec<Value> {
    let (asked_file, answered) = (
        dir(data).join("block.json"),
        dir(data).join("block.out.json"),
    );
    std::fs::write(&asked_file, asked(ids, block).to_string()).unwrap();
    let _ = std::fs::remove_file(&answered);
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args([
        "trial_block",
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads",
        "1",
    ]);
    child
        .env("GATHR_TRIAL_DATA", data)
        .env("GATHR_TRIAL_BLOCK", &asked_file);
    match block.arm {
        Arm::With => child.env_remove("GATHR_TRIAL_WITHOUT"),
        Arm::Without => child.env("GATHR_TRIAL_WITHOUT", "1"),
    };
    let status = child.status().unwrap();
    assert!(
        status.success(),
        "block {} wasn't written: {status}",
        block.n
    );
    let written: Value = serde_json::from_str(&std::fs::read_to_string(answered).unwrap()).unwrap();
    written["runs"].as_array().cloned().unwrap()
}

/// The block's writes, in the child process (`trial_block`): each run's chat in the project with its
/// two agents, T1's work task and T2's review from Ana, and T8's message from her.
pub fn write(data: &Path, asked: &Value) -> Value {
    let folder = gathr::data::DataDir::new(data);
    let _lock = folder
        .take_lock()
        .expect("the service is stopped while a block is written");
    let mut conn = gathr::db::open(&folder.db()).unwrap();
    let trace = gathr::ids::trace_id();
    let ana = asked["ana"].as_str().unwrap();
    gathr::pauses::pause(&conn, "agent", ana, false, &trace).unwrap();
    let at = (asked["project"].as_str(), asked["folder"].as_str());
    project::forget(Path::new(at.1.unwrap()));
    let mut written = Vec::new();
    for run in asked["runs"].as_array().unwrap() {
        let (case, agent) = (
            run["case"].as_str().unwrap(),
            run["agent"].as_str().unwrap(),
        );
        // One title for every chat: the briefing names it, and it says nothing of the trial.
        let chat = gathr::store::create_chat(&conn, "Durations", at).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let seats = match case {
            "T5" => [(agent, Role::Supervisor), (ana, Role::Member)],
            "T2" => [(ana, Role::Supervisor), (agent, Role::Reviewer)],
            _ => [(ana, Role::Supervisor), (agent, Role::Member)],
        };
        let seats: Vec<(String, Role)> = seats.iter().map(|(a, r)| (a.to_string(), *r)).collect();
        assert_eq!(gathr::team::refused(&seats), None);
        gathr::team::set_members(&mut conn, &chat, &seats).unwrap();
        let give = |kind, what, brief| Give {
            chat: &chat,
            giver: Some(ana),
            owner: agent,
            given_in: None,
            parent: None,
            kind,
            what,
            brief,
            after: &[],
            pr: None,
            in_passing: false,
            look_into: false,
        };
        let (mut task, mut message) = (None, None);
        match case {
            "T1" => {
                let brief = Brief {
                    why: Some(T1_WHY),
                    done: Some(T1_DONE),
                    proof: Some(T1_PROOF),
                    place: Some(T1_WHERE),
                    size: None,
                };
                task = Some(
                    gathr::tasks::give(&mut conn, &give(Kind::Work, T1_WHAT, brief), &trace)
                        .unwrap()
                        .0
                        .number,
                );
            }
            "T2" => {
                let brief = Brief {
                    why: Some(T2_WHY),
                    ..Brief::default()
                };
                task = Some(
                    gathr::tasks::give(&mut conn, &give(Kind::Review, T2_WHAT, brief), &trace)
                        .unwrap()
                        .0
                        .number,
                );
            }
            "T8" => message = Some(teammate(&conn, &chat, (ana, agent), T8_TEXT)),
            _ => {}
        }
        written.push(json!({ "chat": chat, "task": task, "message": message }));
    }
    json!({ "runs": written })
}

/// Ana's message to the member, queued as the `message` tool queues one: shown as hers, in a turn of
/// hers that has ended, and Gathr's copy of it opening the member's turn. The shown message's id.
fn teammate(conn: &Connection, chat: &str, (from, to): (&str, &str), text: &str) -> String {
    let (turn, trace) = (
        gathr::ids::ulid(gathr::ids::now_us()),
        gathr::ids::trace_id(),
    );
    gathr::store::add_turn(
        conn,
        chat,
        &turn,
        &trace,
        (Some(from), None, None),
        Opened::Person,
    )
    .unwrap();
    let shown = gathr::store::add_message(conn, chat, &turn, "assistant", "text", text).unwrap();
    let id = shown["id"].as_str().unwrap().to_string();
    conn.execute("UPDATE messages SET to_agent = ?2 WHERE id = ?1", [&id, to])
        .unwrap();
    gathr::store::set_turn(conn, &turn, "done", None).unwrap();
    let told = gathr::handoffs::told(GIVER, &id, text);
    let start = gathr::tasks::Start::queue(
        conn,
        (chat, to),
        (None, Some(&turn)),
        Opened::Teammate,
        told,
    )
    .unwrap();
    let copy = start.message["id"].as_str().unwrap();
    conn.execute(
        "UPDATE messages SET copy_of = ?2 WHERE id = ?1",
        [copy, &id],
    )
    .unwrap();
    id
}
