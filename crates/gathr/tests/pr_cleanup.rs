//! A task's merged/closed PR cleans only its owned branch, through the real service and gh edge.
mod common;

use common::{
    TempDir, TestService, call_tool, fake_gh, gathr, git, held_recording, nth, text, until,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Room {
    service: TestService,
    data: TempDir,
    _repos: TempDir,
    repo: PathBuf,
    worktree: PathBuf,
    lead_worktree: PathBuf,
    branch: String,
    head: String,
    chat: String,
}

impl Drop for Room {
    fn drop(&mut self) {
        let _ = std::fs::write(self.data.0.join("go"), "");
    }
}

fn room() -> Room {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let (repo, _) = common::project(&repos.0);
    let answers = data.0.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &data.0.join("go")),
            ("GATHR_REPLAY_ENV", &data.0.join("env.jsonl")),
            ("GATHR_GH_PATH", &fake_gh()),
            ("GATHR_FAKE_GH", &answers),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
            ("GATHR_IDLE_CLOSE_MS", Path::new("300")),
        ],
    );
    let account = service.add_account("Work");
    common::sign_in_account(&data.0, &account);
    let agent = |name: &str| {
        service
            .send(
                "POST",
                "/api/agents",
                json!({"name":name,"account":account}),
            )
            .json()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let project = service
        .send(
            "POST",
            "/api/projects",
            json!({"name":"Work","folder":repo,
        "members":[{"agent":lead,"role":"supervisor"},{"agent":ann,"role":"member"}]}),
        )
        .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let chat = service
        .send("POST", "/api/chats", json!({"project":project}))
        .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({"text":"Start"}),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead_start = nth(&data.0.join("env.jsonl"), 0);
    let token = lead_start["GATHR_AGENT_TOKEN"].as_str().unwrap();
    let given = call_tool(
        service.info.port,
        token,
        "give_task",
        json!({"to":"Ann","what":"Implement it","pr":5}),
    );
    assert_eq!(given["isError"], false, "{}", text(&given));
    let started = nth(&data.0.join("env.jsonl"), 1);
    let worktree = PathBuf::from(started["cwd"].as_str().unwrap());
    let branch = git(&worktree, &["branch", "--show-current"]);
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    Room {
        service,
        data,
        _repos: repos,
        repo,
        worktree,
        branch,
        head,
        chat,
        lead_worktree: PathBuf::from(lead_start["cwd"].as_str().unwrap()),
    }
}

impl Room {
    fn close(&self, state: &str) {
        self.close_repo(state, false);
    }

    fn close_repo(&self, state: &str, fork: bool) {
        let out = json!({"state":state,"headRefOid":self.head,"headRefName":self.branch,
            "baseRefName":"main","isCrossRepository":fork})
        .to_string();
        std::fs::write(
            self.data.0.join("gh.json"),
            json!([{"args":["pr","view","5"],"out":out}]).to_string(),
        )
        .unwrap();
        until(20, "the task's PR settled", || {
            let tasks = self
                .service
                .call("GET", &format!("/api/chats/{}/tasks", self.chat))
                .json();
            tasks["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["pr"] == 5)
                .filter(|t| {
                    t["state"]
                        == if state == "MERGED" {
                            "done"
                        } else {
                            "cancelled"
                        }
                })
                .map(|_| ())
        });
    }
    fn release(&self) {
        std::fs::write(self.data.0.join("go"), "").unwrap();
    }
    fn event(&self, name: &str, why: Option<&str>) -> Value {
        let extra = why
            .map(|w| format!(" AND error_type = '{w}'"))
            .unwrap_or_default();
        let query = format!(
            "SELECT ref,attrs FROM event_log WHERE name = '{name}'{extra} AND json_extract(attrs, '$.branch') = '{}'",
            self.branch
        );
        until(20, &format!("{name} {why:?} for {}", self.branch), || {
            let (ok, rows) = gathr(&self.data.0, &["diag", "query", &query]);
            assert!(ok, "{rows}");
            rows["rows"].as_array().and_then(|r| r.first()).cloned()
        })
    }
}

#[test]
fn a_forks_same_named_branch_does_not_claim_a_local_worktree() {
    let r = room();
    let scratch = PathBuf::from(
        nth(&r.data.0.join("env.jsonl"), 1)["TEMP"][0]
            .as_str()
            .unwrap(),
    );
    r.close_repo("MERGED", true);
    r.release();
    // Engine-close cleanup runs the PR sweep before removing the worker's scratch folder.
    until(
        20,
        "the worker's engine closed and its cleanup swept",
        || (!scratch.exists()).then_some(()),
    );
    assert!(
        r.worktree.join(".git").is_file(),
        "a fork cannot remove the same-named local worktree"
    );
    assert!(!git(&r.repo, &["branch", "--list", &r.branch]).is_empty());
}

#[test]
fn merged_pr_removes_its_worktree_and_branch_but_keeps_other_branches() {
    let r = room();
    r.close("MERGED");
    r.release();
    r.event("gathr.cleanup.remove", None);
    until(20, "PR worktree and local branch removed", || {
        (!r.worktree.exists() && git(&r.repo, &["branch", "--list", &r.branch]).is_empty())
            .then_some(())
    });
    assert!(
        r.lead_worktree.join(".git").is_file(),
        "another branch's worktree stays"
    );
}

#[test]
fn a_closed_pr_keeps_uncommitted_changes_with_a_reference_then_retries() {
    let r = room();
    let notes = r.worktree.join("notes.md");
    std::fs::write(&notes, "not committed").unwrap();
    r.close("CLOSED");
    r.release();
    let skip = r.event("gathr.cleanup.skip", Some("changes"));
    let reference = skip["ref"].as_str().expect("a dirty skip has a reference");
    let (ok, trace) = gathr(&r.data.0, &["diag", "trace", reference]);
    assert!(
        ok && trace.to_string().contains("gathr.cleanup.skip"),
        "{trace}"
    );
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "not committed");
    assert!(
        !git(&r.repo, &["branch", "--list", &r.branch]).is_empty(),
        "the dirty branch stays"
    );
    std::fs::remove_file(notes).unwrap();
    until(
        20,
        "the same cleanup trace records removal after changes are gone",
        || {
            let (ok, trace) = gathr(&r.data.0, &["diag", "trace", reference]);
            assert!(ok, "{trace}");
            trace
                .to_string()
                .contains("gathr.cleanup.remove")
                .then_some(())
        },
    );
}

#[test]
fn a_running_engine_keeps_its_finished_pr_worktree_until_it_closes() {
    let r = room();
    r.close("MERGED");
    let skip = r.event("gathr.cleanup.skip", Some("in_use"));
    assert!(
        skip["ref"].as_str().is_some(),
        "running-engine skip has a reference: {skip}"
    );
    assert!(
        r.worktree.join(".git").is_file(),
        "the running engine's worktree stays"
    );
    assert!(!git(&r.repo, &["branch", "--list", &r.branch]).is_empty());
    r.release();
    r.event("gathr.cleanup.remove", None);
}

#[test]
fn a_finished_pr_cannot_delete_newer_commits_on_its_reused_branch() {
    let r = room();
    std::fs::write(r.worktree.join("later.md"), "next feature").unwrap();
    git(&r.worktree, &["add", "later.md"]);
    git(&r.worktree, &["commit", "-q", "-m", "later"]);
    let later = git(&r.worktree, &["rev-parse", "HEAD"]);
    r.close("CLOSED");
    r.release();
    r.event("gathr.cleanup.skip", Some("changes"));
    assert_eq!(
        git(&r.worktree, &["rev-parse", "HEAD"]),
        later,
        "new commits stay"
    );
}

#[test]
fn a_pr_branch_checked_out_outside_recorded_agent_work_is_not_deleted() {
    let r = room();
    let outside = r._repos.0.join("personal-worktree");
    git(
        &r.repo,
        &[
            "worktree",
            "add",
            "--force",
            outside.to_str().unwrap(),
            &r.branch,
        ],
    );
    r.close("MERGED");
    r.release();
    until(20, "the owned PR worktree removed", || {
        (!r.worktree.exists()).then_some(())
    });
    r.event("gathr.cleanup.skip", Some("in_use"));
    assert!(outside.join(".git").is_file(), "unrecorded worktree stays");
    assert!(
        !git(&r.repo, &["branch", "--list", &r.branch]).is_empty(),
        "the checked-out branch stays"
    );
}

#[test]
fn pending_pr_cleanup_survives_a_service_restart_after_the_task_settled() {
    let mut r = room();
    let notes = r.worktree.join("keep-until-restart.md");
    std::fs::write(&notes, "not committed").unwrap();
    r.close("MERGED");
    r.event("gathr.cleanup.skip", Some("in_use"));
    let stopped = r.service.call("POST", "/api/shutdown");
    assert_eq!(stopped.status, 202, "{}", stopped.body);
    assert!(r.service.child.wait().unwrap().success());
    assert!(
        notes.is_file(),
        "dirty work guarantees cleanup is still pending at the stop"
    );
    std::fs::remove_file(notes).unwrap();
    r.release();
    let _again = TestService::with_engine(
        &r.data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&r.data.0)),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
            ("GATHR_GH_PATH", &fake_gh()),
            ("GATHR_FAKE_GH", &r.data.0.join("gh.json")),
        ],
    );
    r.event("gathr.cleanup.remove", None);
    until(
        20,
        "persisted cleanup deleted the settled PR's local branch",
        || {
            (!r.worktree.exists() && git(&r.repo, &["branch", "--list", &r.branch]).is_empty())
                .then_some(())
        },
    );
}
