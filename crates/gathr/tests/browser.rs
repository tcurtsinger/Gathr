//! The browser beside the chat (`gathr::browser`): the mistake to guard is losing the person's
//! sign-ins, so the browser must keep its profile under the data folder, come back when it dies, and
//! end with the service. It starts headless and private here (`GATHR_BROWSER_PRIVATE`), as on a PC
//! with an account no test browser may sign in.

mod common;

use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use common::update::events;
use common::{TempDir, TestService, kill, until};

#[test]
fn the_browser_keeps_its_profile_comes_back_when_it_dies_and_ends_with_the_service() {
    if gathr::browser::find().is_none() {
        eprintln!("SKIPPED: no Edge or Chrome on this computer");
        return;
    }
    let dir = TempDir::new();
    let service = TestService::start_with(&dir.0, &[("GATHR_BROWSER_PRIVATE", Path::new("1"))]);
    let mut socket = browser_socket(&service);
    send(
        &mut socket,
        r#"{"type":"view","width":800,"height":600,"ratio":1,"visible":true}"#,
    );

    // Each start is waited for as the service saw it: a browser just started may still change its own process.
    let started = |n: usize| {
        until(60, "the browser to start", || {
            let starts = events(&dir.0, "gathr.browser.start");
            (starts.iter().filter(|e| e["status"] == "ok").count() >= n).then_some(())
        })
    };
    started(1);
    let first = until(20, "the browser's process", || main_browser(&dir.0));
    assert!(
        dir.0.join("browser").is_dir(),
        "the profile is a folder under the data folder"
    );

    kill(&first);
    let crashes = until(60, "the crash to be noted", || {
        Some(events(&dir.0, "gathr.browser.crash")).filter(|c| !c.is_empty())
    });
    assert_eq!(crashes.len(), 1, "{crashes:?}");
    assert_eq!(crashes[0]["error_type"], "exited", "{crashes:?}");
    started(2);
    let second = until(20, "the browser's new process", || {
        main_browser(&dir.0).filter(|pid| *pid != first)
    });

    drop(socket);
    assert!(service.stop().success());
    until(20, "every browser process to end with the service", || {
        running(&dir.0).is_empty().then_some(())
    });
    let stops = events(&dir.0, "gathr.browser.stop");
    assert_eq!(stops.len(), 1, "{stops:?} (second start: {second})");
}

/// A page's own alert waits for the person: shown in the page's state, and once answered over the socket
/// the page goes on. Never answered, the page would stay stopped for good.
#[test]
fn a_pages_alert_waits_for_the_persons_answer_and_then_the_page_goes_on() {
    if gathr::browser::find().is_none() {
        eprintln!("SKIPPED: no Edge or Chrome on this computer");
        return;
    }
    let site = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", site.local_addr().unwrap());
    std::thread::spawn(move || {
        let page = "<!doctype html><title>Before</title>\
                    <script>alert('A made-up alert'); document.title = 'Went on';</script>";
        for stream in site.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut asked = [0u8; 2048];
            let _ = stream.read(&mut asked);
            let answer = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                page.len()
            );
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    let dir = TempDir::new();
    let service = TestService::start_with(&dir.0, &[("GATHR_BROWSER_PRIVATE", Path::new("1"))]);
    let mut socket = browser_socket(&service);
    send(
        &mut socket,
        r#"{"type":"view","width":800,"height":600,"ratio":1,"visible":true}"#,
    );
    send(
        &mut socket,
        &format!(r#"{{"type":"open","address":"{address}"}}"#),
    );

    let page = state_where(&mut socket, |page| !page["dialog"].is_null());
    assert_eq!(page["dialog"]["kind"], "alert", "{page}");
    assert_eq!(page["dialog"]["message"], "A made-up alert", "{page}");
    let id = page["id"].as_str().unwrap().to_string();
    send(
        &mut socket,
        &format!(r#"{{"type":"answer","page":"{id}","accept":true}}"#),
    );
    let page = state_where(&mut socket, |page| page["title"] == "Went on");
    assert!(page["dialog"].is_null(), "{page}");

    drop(socket);
    assert!(service.stop().success());
}

/// The first page in a state message from the service that `found` holds for, waiting at most 60 s.
fn state_where(
    socket: &mut std::net::TcpStream,
    found: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!left.is_zero(), "waited 60 s for the page");
        socket.set_read_timeout(Some(left)).unwrap();
        let (text, payload) = receive(socket);
        let Some(state) = text
            .then(|| serde_json::from_slice::<serde_json::Value>(&payload).ok())
            .flatten()
            .filter(|s| s["type"] == "state")
        else {
            continue;
        };
        if let Some(page) = state["pages"]
            .as_array()
            .and_then(|p| p.iter().find(|p| found(p)))
        {
            return page.clone();
        }
    }
}

/// One frame from the service, unmasked: whether it is text, and its payload.
fn receive(socket: &mut std::net::TcpStream) -> (bool, Vec<u8>) {
    let mut head = [0u8; 2];
    socket.read_exact(&mut head).unwrap();
    let len = match head[1] & 0x7f {
        126 => {
            let mut n = [0u8; 2];
            socket.read_exact(&mut n).unwrap();
            u16::from_be_bytes(n) as usize
        }
        127 => {
            let mut n = [0u8; 8];
            socket.read_exact(&mut n).unwrap();
            u64::from_be_bytes(n) as usize
        }
        n => n as usize,
    };
    let mut payload = vec![0u8; len];
    socket.read_exact(&mut payload).unwrap();
    (head[0] & 0x0f == 1, payload)
}

/// The browser's own process: started with this data folder's profile, not one of its helpers and not
/// the relay.
fn main_browser(data: &Path) -> Option<String> {
    running(data)
        .into_iter()
        .find(|(_, line)| {
            line.contains("--user-data-dir")
                && !line.contains("--type=")
                && !line.contains("--browser-relay")
        })
        .map(|(pid, _)| pid)
}

/// Every process whose command line names this data folder, with its id.
fn running(data: &Path) -> Vec<(String, String)> {
    let name = data.file_name().unwrap().to_string_lossy().to_string();
    let listed = if cfg!(windows) {
        let list = "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId) $($_.CommandLine)\" }";
        Command::new("powershell")
            .args(["-NoProfile", "-Command", list])
            .output()
    } else {
        Command::new("ps")
            .args(["-axww", "-o", "pid=,command="])
            .output()
    };
    let listed = String::from_utf8_lossy(&listed.unwrap().stdout).to_string();
    listed
        .lines()
        .filter(|line| line.contains(&name) && !line.contains("Get-CimInstance"))
        .filter_map(|line| {
            let (pid, rest) = line.trim().split_once(' ')?;
            Some((pid.to_string(), rest.to_string()))
        })
        .collect()
}

/// The Browser tab's socket, past its handshake.
fn browser_socket(service: &TestService) -> std::net::TcpStream {
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", service.info.port)).unwrap();
    let handshake = format!(
        "GET /api/browser?token={} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        service.info.token, service.info.port
    );
    socket.write_all(handshake.as_bytes()).unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8];
    while !head.ends_with(b"\r\n\r\n") {
        socket.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head);
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    socket
}

/// One text frame, masked as a client's must be.
fn send(socket: &mut std::net::TcpStream, text: &str) {
    let mask = [1u8, 2, 3, 4];
    let mut frame = vec![0x81, 0x80 | text.len() as u8];
    frame.extend_from_slice(&mask);
    frame.extend(text.bytes().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    socket.write_all(&frame).unwrap();
}
