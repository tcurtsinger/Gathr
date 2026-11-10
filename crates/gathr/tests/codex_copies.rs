//! The owned Codex copy, through the real API and a local npm metadata/archive feed.
mod common;
use common::{TempDir, TestService};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn checked(service: &TestService, after: Option<&str>) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let answer = service.call("GET", "/api/engines/codex");
        assert_eq!(
            answer.status, 200,
            "Codex engine route missing: {}",
            answer.body
        );
        let copy = answer.json()["copy"].clone();
        if copy["state"] == "idle" && copy["checked"].as_str().is_some_and(|s| Some(s) != after) {
            return copy;
        }
        assert!(
            Instant::now() < deadline,
            "Codex copy did not finish: {copy}"
        );
        std::thread::yield_now();
    }
}

#[test]
fn codex_has_a_separate_owned_copy_and_a_bad_archive_never_replaces_it() {
    let data = TempDir::new();
    let feed = common::feed::Feed::start();
    // A tarball with the same vendor tree as the npm platform package.
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let binary = if cfg!(windows) { "codex.exe" } else { "codex" };
    let package = data.0.join("package");
    let vendor = package.join("vendor").join(target).join("bin");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::copy(common::replay_engine(), vendor.join(binary)).unwrap();
    let archive = data.0.join("codex.tgz");
    let made = std::process::Command::new(gathr::platform::tar())
        .args(["-czf"])
        .arg(&archive)
        .arg("-C")
        .arg(&data.0)
        .arg("package")
        .status()
        .unwrap();
    assert!(made.success(), "test archive was not made");
    let bytes = std::fs::read(&archive).unwrap();
    // Independent system implementation supplies the fixture's npm integrity.
    let checksum = integrity(&archive);
    let target = gathr::platform::release_target().unwrap();
    let publish = |version: &str, integrity: &str| {
        feed.put(
            "latest",
            json!({"version":version}).to_string().into_bytes(),
        );
        feed.put(&format!("{version}-{target}"), json!({"version":format!("{version}-{target}"), "dist":{"tarball":format!("{}/codex.tgz",feed.url().display()), "integrity":integrity}}).to_string().into_bytes());
    };
    feed.put("codex.tgz", bytes);
    publish("0.166.0", &checksum);
    let service = TestService::start_with(&data.0, &[("GATHR_CODEX_RELEASES", &feed.url())]);
    let first = checked(&service, None);
    assert_eq!(first["version"], "0.166.0", "{first}");
    let found = service.call("GET", "/api/engines/codex").json();
    assert!(
        found["found"].as_str().unwrap().contains("engines"),
        "not Gathr's copy: {found}"
    );
    publish("0.167.0", &format!("sha512-{}", "A".repeat(86) + "=="));
    assert_eq!(
        service.call("POST", "/api/engines/codex/update").status,
        200
    );
    let bad = checked(&service, first["checked"].as_str());
    assert_eq!(
        bad["version"], "0.166.0",
        "bad copy replaced current: {bad}"
    );
    assert!(
        bad["failed"]["why"].as_str().unwrap().contains("SHA-512"),
        "{bad}"
    );
    assert!(
        bad["failed"]["ref"].is_string(),
        "failure has no reference: {bad}"
    );
}

fn integrity(path: &std::path::Path) -> String {
    // Node is part of this repository's toolchain; it is used only to prepare this test's feed.
    let out=std::process::Command::new("node").args(["-e", "const fs=require('fs'),c=require('crypto');process.stdout.write('sha512-'+c.createHash('sha512').update(fs.readFileSync(process.argv[1])).digest('base64'))"]).arg(path).output().unwrap();
    assert!(out.status.success(), "fixture checksum failed");
    String::from_utf8(out.stdout).unwrap()
}
