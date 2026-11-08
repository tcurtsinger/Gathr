//! Gathr installed: the service serves its own UI, a browser gets in through a one-time link,
//! and the room's dev data stays apart from the installed app's (0004, 0012).
mod common;

use std::process::Command;

use common::{TempDir, TestService, gathr};
use gathr::data::{DataDir, ServiceInfo};
use gathr::http;

/// A built UI: what `vite build` leaves in `ui/dist`.
fn built_ui(dir: &TempDir) -> std::path::PathBuf {
    let ui = dir.0.join("ui");
    std::fs::create_dir_all(ui.join("assets")).unwrap();
    std::fs::write(ui.join("index.html"), "<!doctype html><title>Gathr</title>").unwrap();
    std::fs::write(ui.join("assets/index-1a2b.js"), "console.log(1)").unwrap();
    ui
}

fn header<'a>(response: &'a http::Response, name: &str) -> Option<&'a str> {
    response.headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[test]
fn the_service_serves_its_ui_and_a_one_time_link_signs_a_browser_in() {
    let dir = TempDir::new();
    let ui = built_ui(&dir);
    let data = dir.0.join("data");
    let service = TestService::start_args(&data, &[("--ui", ui.as_path())]);
    let port = service.info.port;

    let page = service.get("/", &[]);
    assert_eq!(page.status, 200, "{}", page.body);
    assert!(page.body.contains("<title>Gathr</title>"), "{}", page.body);
    let script = service.get("/assets/index-1a2b.js", &[]);
    assert_eq!(script.status, 200);
    assert_eq!(header(&script, "content-type"), Some("text/javascript"));
    assert_eq!(service.get("/../data/service.json", &[]).status, 404);
    assert_eq!(service.get("/api/chats", &[]).status, 401);

    let code = service.call("POST", "/api/once").json()["code"]
        .as_str()
        .unwrap()
        .to_string();
    let signed_in = service.get(&format!("/?once={code}"), &[]);
    assert_eq!(signed_in.status, 303, "{}", signed_in.body);
    assert_eq!(header(&signed_in, "location"), Some("/"));
    let cookie = header(&signed_in, "set-cookie").unwrap().to_string();
    let name = format!("gathr-{port}=");
    assert!(cookie.starts_with(&name), "{cookie}");
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"),
        "{cookie}"
    );
    let pair = cookie.split(';').next().unwrap();
    assert!(
        !pair.contains(&service.info.token),
        "the cookie isn't the token"
    );

    let chats = service.get("/api/chats", &[("Cookie", &format!("theme=dark; {pair}"))]);
    assert_eq!(chats.status, 200, "{}", chats.body);
    let other_port = pair.replace(&name, &format!("gathr-{}=", port + 1));
    assert_eq!(
        service.get("/api/chats", &[("Cookie", &other_port)]).status,
        401
    );
    let tools = service.get("/mcp", &[("Cookie", pair)]);
    assert_eq!(tools.status, 401, "the cookie opened /mcp: {}", tools.body);

    let again = service.get(&format!("/?once={code}"), &[]);
    assert_eq!(again.status, 401, "a link works once");
    assert_eq!(header(&again, "set-cookie"), None);
    let reference = again.json()["ref"].as_str().unwrap().to_string();
    let (ok, trace) = gathr(&data, &["diag", "trace", &reference]);
    assert!(ok, "{trace}");
    assert!(
        trace
            .to_string()
            .contains("the link was used or has expired"),
        "{trace}"
    );
}

/// `gathr` with no `--data`, its app data folder moved into `home` so nothing real is touched.
fn gathr_without_data(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gathr"))
        .args(args)
        .env("APPDATA", home)
        .env("HOME", home)
        .output()
        .unwrap()
}

#[test]
fn seed_and_dev_refuse_without_a_data_folder() {
    let home = TempDir::new();
    for args in [&["seed"][..], &["dev", "--port", "0"]] {
        let out = gathr_without_data(&home.0, args);
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?} ran: {said}");
        assert!(said.contains("--data is required"), "{args:?}: {said}");
    }
    let made: Vec<_> = std::fs::read_dir(&home.0).unwrap().collect();
    assert!(made.is_empty(), "wrote into the app data folder: {made:?}");
}

/// Stops the service a test started some other way than `TestService`.
struct Running(std::path::PathBuf);

impl Drop for Running {
    fn drop(&mut self) {
        if let Ok(info) = ServiceInfo::read(&DataDir::new(&self.0)) {
            let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
        }
    }
}

#[test]
fn open_starts_the_service_without_dev_variables_and_leaves_it_running() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    let _running = Running(data.clone());
    // To a file, not a pipe: on Windows the service it starts inherits the pipe and holds it open.
    let answer = dir.0.join("open.json");
    let open = || {
        let status = Command::new(env!("CARGO_BIN_EXE_gathr"))
            .arg("--data")
            .arg(&data)
            .arg("open")
            .env("GATHR_CLAUDE_PATH", common::replay_engine())
            .stdout(std::fs::File::create(&answer).unwrap())
            .status()
            .unwrap();
        let said = std::fs::read_to_string(&answer).unwrap();
        assert!(status.success(), "{said}");
        serde_json::from_str::<serde_json::Value>(&said).unwrap()
    };

    let first = open();
    assert_eq!(first["started"], true, "{first}");
    let info = ServiceInfo::read(&DataDir::new(&data)).unwrap();
    let engine = http::call(info.port, &info.token, "GET", "/api/engines/claude").unwrap();
    assert!(!engine.body.contains("replay_engine"), "{}", engine.body);

    let url = first["url"].as_str().unwrap();
    let path = url
        .strip_prefix(&format!("http://127.0.0.1:{}", info.port))
        .unwrap();
    let signed_in = http::request(info.port, "GET", path, &[]).unwrap();
    assert_eq!(signed_in.status, 303, "{}", signed_in.body);

    let second = open();
    assert_eq!(second["started"], false, "{second}");
    assert_eq!(
        ServiceInfo::read(&DataDir::new(&data)).unwrap().pid,
        info.pid
    );
}

#[test]
fn the_service_keeps_off_the_rooms_dev_ports() {
    let dir = TempDir::new();
    let data = DataDir::new(dir.0.join("data"));
    std::fs::create_dir_all(data.path()).unwrap();
    let free = (47100..=47114)
        .find(|&p| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok())
        .expect("a free dev port");
    let last = ServiceInfo {
        port: free,
        token: "old".into(),
        pid: 0,
    };
    last.write(&data).unwrap();
    let _running = Running(data.path().to_path_buf());
    let mut child = Command::new(env!("CARGO_BIN_EXE_gathrd"))
        .arg("--data")
        .arg(data.path())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let info = loop {
        match ServiceInfo::read(&data) {
            Ok(info) if info.pid == child.id() => break info,
            _ if matches!(child.try_wait(), Ok(Some(_))) => panic!("gathrd exited"),
            _ if std::time::Instant::now() > deadline => panic!("gathrd didn't start in 20 s"),
            _ => std::thread::yield_now(),
        }
    };
    assert!(
        !(47100..=47114).contains(&info.port),
        "took dev port {}",
        info.port
    );
}

#[test]
fn open_says_at_once_why_the_service_failed_to_start() {
    let dir = TempDir::new();
    let data = dir.0.join("data");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("gathr.db"), "not a database").unwrap();
    let started = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_gathr"))
        .arg("--data")
        .arg(&data)
        .arg("open")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(said.contains("the database didn't open"), "{said}");
    // It used to wait 15 s for some other service before saying so.
    let waited = started.elapsed();
    assert!(
        waited < std::time::Duration::from_secs(10),
        "took {waited:?}"
    );
}
