//! 0036 C3: Gathr runs a project's checks at a commit's exact contents, in its own checkout reset
//! for each run, with caches kept outside it; a failure carries a reference.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, git, project, until};
use serde_json::{Value, json};

struct Repo {
    service: TestService,
    project: String,
    /// The project's own checkout, where the person (or an agent) leaves things uncommitted.
    folder: PathBuf,
    /// A commit pushed to the project's origin on a branch of its own.
    head: String,
}

/// A project whose origin has a branch with one more commit than its default branch.
fn repo(data: &Path, repos: &Path) -> Repo {
    let (folder, _) = project(repos);
    let other = repos.join("other");
    git(&other, &["checkout", "-q", "-b", "room/x"]);
    std::fs::write(other.join("feature.txt"), "made up\n").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-q", "-m", "feature"]);
    git(&other, &["push", "-q", "origin", "room/x"]);
    let head = git(&other, &["rev-parse", "HEAD"]).trim().to_string();
    let service = TestService::start(data);
    let account = service.add_account("Team");
    let agent = service
        .send(
            "POST",
            "/api/agents",
            json!({ "name": "Lead", "account": account }),
        )
        .json()["id"]
        .clone();
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let body = json!({ "name": "Site", "folder": folder, "members": members });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    Repo {
        project: made.json()["id"].as_str().unwrap().to_string(),
        service,
        folder,
        head,
    }
}

impl Repo {
    /// Sets the project's checks as its Supervisor would (C2b).
    fn checks(&self, data: &Path, commands: &[&str]) {
        let db = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
        db.execute(
            "DELETE FROM project_checks WHERE project_id = ?1",
            [&self.project],
        )
        .unwrap();
        for (i, command) in commands.iter().enumerate() {
            db.execute(
                "INSERT INTO project_checks (project_id, position, command, source, set_ts)
                 VALUES (?1, ?2, ?3, 'found', 1)",
                rusqlite::params![self.project, i as i64, command],
            )
            .unwrap();
        }
    }

    /// Runs the checks at the head and waits for the run to end: the run as the API shows it.
    fn run(&self) -> Value {
        let path = format!("/api/projects/{}/checks/runs", self.project);
        let started = self
            .service
            .send("POST", &path, json!({ "head": self.head }));
        assert_eq!(started.status, 200, "{}", started.body);
        let id = started.json()["run"].as_str().unwrap().to_string();
        until(60, "the run to end", || {
            let runs = self.service.call("GET", &path).json();
            let run = runs["runs"]
                .as_array()?
                .iter()
                .find(|r| r["run"] == id.as_str())?
                .clone();
            (run["state"] != "running").then_some(run)
        })
    }
}

fn logs(run: &Value) -> String {
    let steps = run["steps"].as_array().unwrap().iter();
    steps
        .map(|s| s["log"].as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_run_sees_the_commit_alone_never_what_is_left_uncommitted() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let repo = repo(&data.0, &repos.0);
    std::fs::write(repo.folder.join("uncommitted.txt"), "not in the commit\n").unwrap();
    repo.checks(
        &data.0,
        &[
            "test -f feature.txt",
            "test ! -f uncommitted.txt",
            "echo left > left.txt",
        ],
    );
    let first = repo.run();
    assert_eq!(first["state"], "passed", "{first}");
    assert_eq!(first["head"], repo.head.as_str(), "{first}");
    assert_eq!(first["steps"].as_array().unwrap().len(), 3, "{first}");
    // What a run leaves behind is gone before the next one.
    repo.checks(&data.0, &["test ! -f left.txt"]);
    let second = repo.run();
    assert_eq!(
        second["state"], "passed",
        "a run saw the last one's files: {second}"
    );
}

#[test]
fn a_failing_command_fails_the_run_and_its_reference_traces() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let repo = repo(&data.0, &repos.0);
    repo.checks(
        &data.0,
        &["echo first", "echo broken >&2; exit 3", "echo never"],
    );
    let run = repo.run();
    assert_eq!(run["state"], "failed", "{run}");
    let steps = run["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "it went on after a failure: {run}");
    assert_eq!(steps[1]["exit"], 3, "{run}");
    assert!(
        steps[1]["log"].as_str().unwrap().contains("broken"),
        "{run}"
    );
    let reference = run["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let trace = trace.to_string();
    assert!(
        ok && trace.contains("gathr.checks.end") && trace.contains("exit 3"),
        "{trace}"
    );
}

#[test]
fn a_second_run_reuses_the_caches_and_removing_them_empties_them() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let repo = repo(&data.0, &repos.0);
    let reuse = r#"if test -f "$CARGO_TARGET_DIR/seen"; then echo reused; else mkdir -p "$CARGO_TARGET_DIR" && echo kept > "$CARGO_TARGET_DIR/seen" && echo fresh; fi"#;
    repo.checks(&data.0, &[reuse]);
    assert!(logs(&repo.run()).contains("fresh"));
    assert!(
        logs(&repo.run()).contains("reused"),
        "the second run started cold"
    );

    let path = format!("/api/projects/{}/caches", repo.project);
    let size = repo.service.call("GET", &path).json();
    assert!(size["bytes"].as_u64().unwrap() > 0, "{size}");
    assert_eq!(repo.service.call("DELETE", &path).status, 200);
    assert_eq!(repo.service.call("GET", &path).json()["bytes"], 0);
    assert!(
        logs(&repo.run()).contains("fresh"),
        "removed caches came back"
    );
}

#[test]
fn a_project_without_checks_has_nothing_to_run() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let repo = repo(&data.0, &repos.0);
    let path = format!("/api/projects/{}/checks/runs", repo.project);
    let refused = repo
        .service
        .send("POST", &path, json!({ "head": repo.head }));
    assert_eq!(refused.status, 409, "{}", refused.body);
    assert!(refused.json()["ref"].is_string(), "{}", refused.body);
}
