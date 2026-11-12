//! Gathr updates its own service between turns (docs/decisions/0032-service-updates.md), through
//! copies of the real `gathrd` in a test data folder and a stand-in release feed on 127.0.0.1.
mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::feed::Feed;
use common::{TempDir, TestService, gathr, held_recording, recording, replay_engine, until};
use gathr::data::{DataDir, ServiceInfo};
use gathr::{VERSION, http};
use serde_json::{Value, json};

fn program_name() -> String {
    format!("gathrd{}", std::env::consts::EXE_SUFFIX)
}

#[test]
fn adopting_an_install_keeps_its_diagnostics_cli_with_the_service() {
    let dir = TempDir::new();
    let installed = dir.0.join("installed");
    std::fs::create_dir_all(&installed).unwrap();
    let service = installed.join(program_name());
    std::fs::copy(env!("CARGO_BIN_EXE_gathrd"), &service).unwrap();
    let cli_name = format!("gathr{}", std::env::consts::EXE_SUFFIX);
    std::fs::copy(env!("CARGO_BIN_EXE_gathr"), installed.join(&cli_name)).unwrap();
    let data = DataDir::new(dir.0.join("data"));
    let copies = gathr::app::Copies::new(&data);
    copies.adopt(&service).unwrap();
    let cli = copies.path(VERSION).with_file_name(cli_name);
    let output = std::process::Command::new(&cli)
        .args(["diag", "health", "--data"])
        .arg(data.path())
        .output()
        .unwrap_or_else(|e| panic!("adopted diagnostics CLI {} did not run: {e}", cli.display()));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let health: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        health["database"]["path"],
        data.db().to_string_lossy().as_ref()
    );
}

/// The built `gathrd` as the folder's copy of `version`, named current: what an older install left.
fn seed_copy(data: &Path, version: &str) -> PathBuf {
    let folder = data.join("app").join(version);
    std::fs::create_dir_all(&folder).unwrap();
    let program = folder.join(program_name());
    std::fs::copy(env!("CARGO_BIN_EXE_gathrd"), &program).unwrap();
    let current = json!({ "version": version }).to_string();
    std::fs::write(data.join("app/current.json"), current).unwrap();
    program
}

/// Publishes `version` as the newest release: `program` as its `gathrd`, with a UI saying `ui`.
fn publish(feed: &Feed, dir: &Path, version: &str, program: &str, ui: &str) {
    let stage = dir.join(format!("stage-{}", gathr::ids::span_id()));
    std::fs::create_dir_all(stage.join("ui")).unwrap();
    std::fs::copy(program, stage.join(program_name())).unwrap();
    std::fs::write(stage.join("ui/index.html"), ui).unwrap();
    let archive = stage.with_extension("tar.gz");
    let made = std::process::Command::new(gathr::platform::tar())
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&stage)
        .args([program_name().as_str(), "ui"])
        .status()
        .unwrap();
    assert!(made.success(), "tar: {made}");
    let bytes = std::fs::read(&archive).unwrap();
    let target = gathr::platform::release_target().expect("a platform Gathr ships for");
    let file = format!("gathrd-{version}-{target}.tar.gz");
    let manifest = json!({ "version": version, "platforms": { target: {
        "file": file, "sha256": gathr::fetch::sha256(&bytes), "size": bytes.len() } } });
    feed.put(&file, bytes);
    feed.put("gathrd.json", manifest.to_string().into_bytes());
}

/// The service that took over from `pid` on the folder, once it answers.
fn successor(data: &Path, pid: u32) -> ServiceInfo {
    until(90, "a new service on the folder", || {
        let info = ServiceInfo::read(&DataDir::new(data)).ok()?;
        let health = http::call(info.port, &info.token, "GET", "/health").ok()?;
        (info.pid != pid && health.status == 200).then_some(info)
    })
}

fn get(info: &ServiceInfo, path: &str) -> Value {
    http::call(info.port, &info.token, "GET", path)
        .unwrap()
        .json()
}

/// Stops whatever service is on the folder when the test ends, as `TestService` stops its own.
struct Successor(PathBuf);

impl Drop for Successor {
    fn drop(&mut self) {
        let Ok(info) = ServiceInfo::read(&DataDir::new(&self.0)) else {
            return;
        };
        let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
        let deadline = Instant::now() + Duration::from_secs(20);
        while http::call(info.port, &info.token, "GET", "/health").is_ok()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn events(data: &Path, name: &str) -> Vec<Value> {
    let sql = format!(
        "SELECT status, error_type, attrs, ref FROM event_log WHERE name = '{name}' ORDER BY id"
    );
    let (ok, rows) = gathr(data, &["diag", "query", &sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

/// Checks for a release now; `GET /api/app` says when it's ready.
fn check_now(service: &TestService) {
    let asked = service.call("POST", "/api/app/update");
    assert_eq!(asked.status, 200, "{}", asked.body);
}

/// Once a release is ready, waits for nothing (manual updates): never switches until asked.
fn ready(service: &TestService) -> Value {
    until(30, "a ready update", || {
        let app = service.call("GET", "/api/app").json();
        (app["state"] == "ready").then_some(app)
    })
}

/// Update in the sidebar's foot: installs the ready release now.
fn install(service: &TestService) -> Value {
    let asked = service.send("POST", "/api/app/update", json!({ "install": true }));
    assert_eq!(asked.status, 200, "{}", asked.body);
    asked.json()
}

/// What the person does: check, then Update once it's ready.
fn update_now(service: &TestService) {
    check_now(service);
    ready(service);
    install(service);
}

fn say(service: &TestService, chat: &str, text: &str) -> Value {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

/// An agent on the signed-in account, and a chat it said `text` in.
fn agent_says(service: &TestService, data: &Path, text: &str) -> (String, String) {
    let account = service.signed_in(data);
    let made = json!({ "name": "Made-up", "account": account });
    let made = service.send("POST", "/api/agents", made);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    (agent, chat)
}

/// The carry-on Update leaves the agent, run on the new service: its words.
fn carried_on(next: &ServiceInfo, chat: &str) -> Value {
    until(30, "the carry-on to run on the new service", || {
        let history = get(next, &format!("/api/chats/{chat}"));
        let turns = history["turns"].as_array()?;
        (turns.len() == 2 && turns[1]["status"] == "done").then_some(history)
    })
}

/// A release is checked and kept ready, never switched to until the person says (the room's
/// manual-update plan). Then the turn under way stops, and its agent carries on by itself on the
/// new version.
#[test]
fn a_ready_release_waits_for_the_person_then_its_agents_carry_on() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let (feed, go) = (Feed::start(), dir.0.join("go"));
    let held = held_recording(&dir.0);
    let service = TestService::start_program(
        &old,
        &data,
        &[
            ("GATHR_APP_RELEASES", &feed.url()),
            ("GATHR_CLAUDE_PATH", &replay_engine()),
            ("GATHR_REPLAY", &held),
            ("GATHR_REPLAY_HOLD", &go),
        ],
    );
    let _next = Successor(data.clone());
    let (_, chat) = agent_says(&service, &data, "Say hi");
    service.wait_for(&chat, 20, "the turn to run", |h| {
        h["turns"][0]["status"] == "running"
    });

    publish(
        &feed,
        &dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathrd"),
        "new ui",
    );
    check_now(&service);
    let app = ready(&service);
    assert_eq!(app["update"]["version"], VERSION, "{app}");
    std::thread::sleep(Duration::from_secs(1));
    let still = ServiceInfo::read(&DataDir::new(&data)).unwrap();
    assert_eq!(still.pid, service.info.pid, "it switched by itself");

    assert_eq!(install(&service)["installing"], VERSION);
    std::fs::write(&go, "").unwrap();
    let next = successor(&data, service.info.pid);
    let app = get(&next, "/api/app");
    assert_eq!(
        (&app["version"], &app["previous"]),
        (&json!(VERSION), &json!("0.0.1")),
        "{app}"
    );
    let page = http::request(next.port, "GET", "/", &[]).unwrap();
    assert!(
        page.body.contains("new ui"),
        "the new copy's UI: {}",
        page.body
    );
    let history = carried_on(&next, &chat);
    assert_eq!(history["turns"][0]["status"], "stopped", "{history}");
    assert_eq!(
        history["turns"][1]["answers"], history["turns"][0]["id"],
        "{history}"
    );
    let told = format!("Gathr's update to {VERSION} ended your turn");
    assert!(history.to_string().contains(&told), "{history}");
    let switched = events(&data, "gathr.app.switch");
    assert_eq!(switched.len(), 1, "{switched:?}");
    assert_eq!(switched[0]["attrs"]["from"], "0.0.1", "{switched:?}");
    // A finished turn passes the trial: nothing goes back after it.
    until(20, "the trial to pass", || {
        (current(&data)["trial"] == false).then_some(())
    });
    let kept = data.join("app").join(format!("gathr.db.before-{VERSION}"));
    assert!(!kept.exists(), "the database copy outlived the trial");
}

/// A background command never holds an update (G194: one held the room 30 minutes): Update ends
/// it as Stop now does, and its agent carries on on the new version.
#[test]
fn a_background_command_never_holds_an_update() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let feed = Feed::start();
    let service = TestService::start_program(
        &old,
        &data,
        &[
            ("GATHR_APP_RELEASES", &feed.url()),
            ("GATHR_CLAUDE_PATH", &replay_engine()),
            ("GATHR_REPLAY", &recording("background")),
            ("GATHR_REPLAY_HOLD", &dir.0.join("go")),
        ],
    );
    let _next = Successor(data.clone());
    let (_, chat) = agent_says(&service, &data, "Run the pings in the background");
    service.wait_for(&chat, 20, "the turn that starts them", |h| {
        h["turns"][0]["status"] == "done"
    });

    publish(
        &feed,
        &dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathrd"),
        "new ui",
    );
    let began = Instant::now();
    update_now(&service);
    let next = successor(&data, service.info.pid);
    let took = began.elapsed();
    assert!(took < Duration::from_secs(40), "the update took {took:?}");
    let history = carried_on(&next, &chat);
    let told = "ended 1 background command(s) of yours";
    assert!(history.to_string().contains(told), "{history}");
    // The carry-on's engine plays the recording again: let its command end.
    std::fs::write(dir.0.join("go"), "").unwrap();
}

/// The window downloads and checks its own update; the service shows it ready, and Update asks
/// the window to install it, through the data folder they share.
#[test]
fn update_asks_the_window_to_install_its_ready_update() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let feed = Feed::start();
    let service = TestService::start_program(&old, &data, &[("GATHR_APP_RELEASES", &feed.url())]);
    let _next = Successor(data.clone());
    let none = service.send("POST", "/api/app/update", json!({ "install": true }));
    assert_eq!(none.status, 409, "nothing is ready: {}", none.body);
    assert!(none.json()["ref"].is_string(), "{}", none.body);

    let version = json!({ "version": "9.9.9" }).to_string();
    std::fs::write(data.join("window-update.json"), version).unwrap();
    let app = ready(&service);
    assert_eq!(app["window"]["version"], "9.9.9", "{app}");
    assert_eq!(install(&service)["window"], "9.9.9");
    assert!(
        data.join("window-install").is_file(),
        "the window wasn't asked"
    );
    std::thread::sleep(Duration::from_millis(500));
    let still = ServiceInfo::read(&DataDir::new(&data)).unwrap();
    assert_eq!(
        still.pid, service.info.pid,
        "the service has no update of its own"
    );
}

fn current(data: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(data.join("app/current.json")).unwrap()).unwrap()
}

/// `gathr open`, as the window starts the folder's service.
fn open(dir: &Path, data: &Path) -> ServiceInfo {
    let answer = dir.join("open.json");
    // To a file, not a pipe: on Windows the service it starts inherits the pipe.
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_gathr"))
        .arg("--data")
        .arg(data)
        .arg("open")
        .stdout(std::fs::File::create(&answer).unwrap())
        .status()
        .unwrap();
    let said = std::fs::read_to_string(&answer).unwrap();
    assert!(status.success(), "{said}");
    ServiceInfo::read(&DataDir::new(data)).unwrap()
}

fn until_free(data: &Path, what: &str) {
    until(30, what, || DataDir::new(data).take_lock().ok().map(drop));
}

/// The new copy starts, then dies before it finishes a turn: the next start goes back to the copy
/// before. `migrated`: it had moved the database on, so the database from before the update comes
/// back too; otherwise what was written on the new copy stays. `resigned`: the folder's copy differs
/// from the installed program of the same version (macOS signs the bundle's again), so the start
/// copies the installed one in again: still on trial.
fn dies_on_trial(migrated: bool, resigned: bool) {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let feed = Feed::start();
    publish(
        &feed,
        &dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathrd"),
        "new ui",
    );
    let mut service =
        TestService::start_program(&old, &data, &[("GATHR_APP_RELEASES", &feed.url())]);
    let _next = Successor(data.clone());
    update_now(&service);
    let new = successor(&data, service.info.pid);
    assert!(service.child.wait().unwrap().success());
    let made = http::send(new.port, &new.token, "POST", "/api/chats", &json!({})).unwrap();
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();

    // Quitting during the trial isn't failing it: the next start runs the new copy again.
    let _ = http::call(new.port, &new.token, "POST", "/api/shutdown");
    until_free(&data, "the new copy to stop");
    let again = open(&dir.0, &data);
    assert_eq!(get(&again, "/api/app")["version"], VERSION);

    common::kill(&again.pid.to_string());
    until_free(&data, "the new copy to die");
    if migrated {
        let db = rusqlite::Connection::open(DataDir::new(&data).db()).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
    }
    if resigned {
        let copy = data.join("app").join(VERSION).join(program_name());
        let mut file = std::fs::OpenOptions::new().append(true).open(copy).unwrap();
        std::io::Write::write_all(&mut file, b"signed again").unwrap();
    }
    let back = open(&dir.0, &data);
    let app = get(&back, "/api/app");
    assert_eq!(app["version"], "0.0.1", "{app}");
    let reference = app["failed"]["ref"].as_str().unwrap_or_default();
    let (ok, trace) = gathr(&data, &["diag", "trace", reference]);
    assert!(ok && trace.to_string().contains("on trial"), "{trace}");
    assert_eq!(current(&data)["refused"], json!([VERSION]));
    let path = format!("/api/chats/{chat}");
    let kept = http::call(back.port, &back.token, "GET", &path).unwrap();
    assert_eq!(kept.status == 200, !migrated, "{}", kept.body);
}

#[test]
fn a_release_that_dies_before_its_first_turn_rolls_back_and_keeps_what_it_wrote() {
    dies_on_trial(false, false);
}

#[test]
fn a_release_that_dies_after_moving_the_database_on_rolls_back_with_the_database_before() {
    dies_on_trial(true, false);
}

#[test]
fn a_release_that_dies_on_trial_rolls_back_though_its_copy_differs_from_the_installed_one() {
    dies_on_trial(false, true);
}

/// The window updated first, so the installed `gathrd` is newer than the folder's copy: it's taken
/// in on trial with the database kept, so a crash after moving the schema on rolls both back. The
/// old service died without its checkpoint, so its last write is still in the WAL: the kept copy
/// has it too.
#[test]
fn a_newer_installed_copy_that_dies_after_moving_the_database_on_rolls_back_with_it() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let chat = {
        let service = TestService::start_program(&old, &data, &[]);
        let made = service.send("POST", "/api/chats", json!({}));
        assert_eq!(made.status, 200, "{}", made.body);
        common::kill(&service.info.pid.to_string());
        until_free(&data, "the old service to die");
        made.json()["id"].as_str().unwrap().to_string()
    };
    let wal = DataDir::new(&data).db().with_extension("db-wal");
    let in_wal = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert!(
        in_wal > 0,
        "the old service's writes are still in {}",
        wal.display()
    );
    let _next = Successor(data.clone());
    let new = open(&dir.0, &data);
    assert_eq!(get(&new, "/api/app")["version"], VERSION);
    common::kill(&new.pid.to_string());
    until_free(&data, "the new copy to die");
    let db = rusqlite::Connection::open(DataDir::new(&data).db()).unwrap();
    db.pragma_update(None, "user_version", 99).unwrap();
    drop(db);

    let back = open(&dir.0, &data);
    assert_eq!(get(&back, "/api/app")["version"], "0.0.1");
    let kept = http::call(back.port, &back.token, "GET", &format!("/api/chats/{chat}")).unwrap();
    assert_eq!(kept.status, 200, "{}", kept.body);
}

/// 0.1.38's hand-over to 0.1.39 hung here: an engine killed at shutdown left children holding its
/// output, and the stopped service waited on that read for good, still holding the folder. The
/// update starts anyway, and records where its hand-over got to.
#[test]
fn a_killed_engines_children_dont_hold_up_the_hand_over() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let (feed, child) = (Feed::start(), dir.0.join("child-lives"));
    std::fs::write(&child, "").unwrap();
    let service = TestService::start_program(
        &old,
        &data,
        &[
            ("GATHR_APP_RELEASES", &feed.url()),
            ("GATHR_CLAUDE_PATH", &replay_engine()),
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_INIT", Path::new("linger")),
            ("GATHR_REPLAY_CHILD", &child),
        ],
    );
    let _next = Successor(data.clone());
    service.signed_in(&data);
    let chat = service.new_chat();
    say(&service, &chat, "Say hi");
    service.wait_for(&chat, 20, "the turn", |h| h["turns"][0]["status"] == "done");

    publish(
        &feed,
        &dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathrd"),
        "new ui",
    );
    let began = Instant::now();
    update_now(&service);
    successor(&data, service.info.pid);
    let took = began.elapsed();
    std::fs::remove_file(&child).unwrap();
    // The engine's 10 s grace and the stopped service's 5 s wait; the child lives a minute.
    assert!(
        took < Duration::from_secs(40),
        "the hand-over took {took:?}"
    );
    let handed = events(&data, "gathr.app.handover");
    assert_eq!(handed.len(), 1, "{handed:?}");
    let attrs = &handed[0]["attrs"];
    assert!(handed[0]["error_type"].is_null(), "{handed:?}");
    assert_eq!(attrs["step"], gathr::app::STARTING, "{attrs}");
    assert_eq!(
        (&attrs["from"], &attrs["to"]),
        (&json!("0.0.1"), &json!(VERSION))
    );
    // Only Windows reads a child's pipes as blocking work.
    assert_eq!(attrs["left_running"], cfg!(windows), "{attrs}");
}

/// The window started the new copy after the old service let go of the folder and before that
/// service handed over: the hand-over finds it running and leaves its trial alone.
#[test]
fn a_hand_over_that_finds_the_new_copy_already_started_leaves_its_trial() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    seed_copy(&data, "0.0.1");
    let folder = DataDir::new(&data);
    let _next = Successor(data.clone());
    let installed = Path::new(env!("CARGO_BIN_EXE_gathrd"));
    let first = gathr::launch::find_or_start(&folder, installed).unwrap();
    assert!(first.started);
    assert_eq!(current(&data)["trial_running"], true);

    let late = gathr::launch::hand_over(&folder).unwrap();
    assert_eq!(late.info.pid, first.info.pid);
    let now = current(&data);
    assert_eq!(now["version"], VERSION, "{now}");
    assert_eq!(now["refused"], json!([]), "{now}");
}

/// The window opens a folder whose lock is held for a moment by no service: it starts the
/// service once the lock is let go, rather than waiting for one to answer.
#[test]
fn a_start_takes_a_folder_held_by_no_service_once_it_is_let_go() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let folder = DataDir::new(&data);
    let _next = Successor(data.clone());
    let held = folder.take_lock().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        drop(held);
    });
    let installed = Path::new(env!("CARGO_BIN_EXE_gathrd"));
    let found = gathr::launch::find_or_start(&folder, installed);
    release.join().unwrap();
    assert!(found.unwrap().started);
}

/// A release whose program says the right version but isn't a service (the command line packed
/// as `gathrd`): its start fails, and the copy before comes back with the database as it was.
#[test]
fn a_release_that_fails_to_start_rolls_back_to_the_copy_before() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let old = seed_copy(&data, "0.0.1");
    let feed = Feed::start();
    publish(
        &feed,
        &dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathr"),
        "bad ui",
    );
    let mut service =
        TestService::start_program(&old, &data, &[("GATHR_APP_RELEASES", &feed.url())]);
    let _next = Successor(data.clone());

    update_now(&service);
    let back = successor(&data, service.info.pid);
    assert!(service.child.wait().unwrap().success());
    let app = get(&back, "/api/app");
    assert_eq!(app["version"], "0.0.1", "{app}");
    let reference = app["failed"]["ref"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let (ok, trace) = gathr(&data, &["diag", "trace", &reference]);
    let traced = trace.to_string();
    assert!(
        ok && traced.contains("exited") && traced.contains(VERSION),
        "{trace}"
    );
    let current = current(&data);
    assert_eq!(current["refused"], json!([VERSION]), "{current}");
    assert!(
        !data.join("app").join(VERSION).exists(),
        "the failed copy was kept"
    );

    let asked = http::call(back.port, &back.token, "POST", "/api/app/update").unwrap();
    assert_eq!(asked.status, 200, "{}", asked.body);
    until(20, "the check to end", || {
        let app = get(&back, "/api/app");
        (app["state"] == "idle" && !app["checked"].is_null()).then_some(())
    });
    let now = ServiceInfo::read(&DataDir::new(&data)).unwrap();
    assert_eq!(now.pid, back.pid, "a refused release was tried again");
    let archives = feed.asked_for(".tar.gz");
    assert_eq!(archives, 1, "a refused release was downloaded again");
}

/// Open at login starts the installed window, which isn't beside the folder's copy of the service
/// (v0.1.9 failed Settings > General on every installed Gathr). Read only: nothing is turned on.
#[test]
fn open_at_login_finds_the_installed_window_from_the_folders_copy() {
    let dir = TempDir::new();
    let installed = dir.0.join("Gathr");
    std::fs::create_dir_all(&installed).unwrap();
    let gathrd = installed.join(program_name());
    std::fs::copy(env!("CARGO_BIN_EXE_gathrd"), &gathrd).unwrap();
    let window = installed.join(format!("gathr-app{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&window, "").unwrap();
    let data = dir.0.join("data");
    let _running = Successor(data.clone());
    let found = gathr::launch::find_or_start(&DataDir::new(&data), &gathrd).unwrap();

    let startup = http::call(found.info.port, &found.info.token, "GET", "/api/startup").unwrap();
    assert_eq!(startup.status, 200, "{}", startup.body);
    assert_eq!(startup.json()["on"], false, "{}", startup.body);

    std::fs::remove_file(&window).unwrap();
    let gone = http::call(found.info.port, &found.info.token, "GET", "/api/startup").unwrap();
    assert_eq!(gone.status, 500, "{}", gone.body);
    let why = gone.json()["why"].as_str().unwrap_or_default().to_string();
    assert!(why.contains(&window.display().to_string()), "{why}");
}
