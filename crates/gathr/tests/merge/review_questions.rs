//! 0053, scenario 10g through the service: a review asks both questions. A done review that skips
//! the unasked question is refused with a reference; a review left with an unasked note holds its
//! PR and gives its builder a fix, and the clean review after it lands the PR.
use super::*;

const NOTE: &str = "feature.txt isn't part of the ask";

impl Room {
    fn task(&self, number: i64) -> Value {
        let tasks = self
            .service
            .call("GET", &format!("/api/chats/{}/tasks", self.chat))
            .json();
        let all = tasks["tasks"].as_array().unwrap();
        all.iter()
            .find(|t| t["number"] == number)
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// The service's looks at PR #5 so far, as GitHub's stand-in counts them.
    fn looks(&self) -> usize {
        let calls = std::fs::read_to_string(self.data.join("gh.json.calls")).unwrap_or_default();
        calls
            .lines()
            .filter(|line| line.contains("headRefOid"))
            .count()
    }
}

/// A reference's trace, which must hold `has`.
fn traced(data: &Path, said: &str, has: &str) {
    let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    assert!(ok && trace.to_string().contains(has), "{trace}");
}

#[test]
fn a_review_that_skips_the_unasked_question_is_refused_with_a_reference() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let done = json!({ "how": "done", "task": 1, "commit": room.heads[0] });
    let refused = room.call(&vale, "finish_task", done);
    let said = text(&refused);
    assert_eq!(refused["isError"], true, "{said}");
    let why = "A done review says nothing_unasked: true and has no wrong or unasked notes. Report those as left.";
    assert!(said.contains(why), "{said}");
    traced(&data.0, &said, "nothing_unasked");
    let odd = json!({ "how": "left", "task": 1, "note": "odd", "commit": room.heads[0],
                      "notes": [{ "kind": "odd", "says": "something" }] });
    let refused = room.call(&vale, "finish_task", odd);
    let said = text(&refused);
    assert_eq!(refused["isError"], true, "{said}");
    assert!(
        said.contains("odd isn't a note's kind: wrong, unasked or suggestion"),
        "{said}"
    );
    traced(&data.0, &said, "isn't a note's kind");
    assert_eq!(room.main(), room.base, "a refusal landed something");
}

#[test]
fn a_review_left_with_an_unasked_note_holds_its_pr_and_a_clean_one_lands_it() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room_replay(&data.0, &repos.0, false, true);
    let mut answers: Value =
        serde_json::from_str(&std::fs::read_to_string(data.0.join("gh.json")).unwrap()).unwrap();
    let default = r#"{"defaultBranchRef":{"name":"main"}}"#;
    answers
        .as_array_mut()
        .unwrap()
        .push(json!({ "args": ["repo", "view"], "out": default }));
    put_answers(&data.0, &answers);
    let lead = room.lead_turn();
    let work = json!({ "to": "Ann", "what": "Add the feature", "pr": 5 });
    let given = text(&room.call(&lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann"), "{given}");
    room.token();
    let vale = room.reviews(&lead, &[5]);
    let vale_cwd = PathBuf::from(nth(&data.0.join("env.jsonl"), 2)["cwd"].as_str().unwrap());

    let left = json!({ "how": "left", "task": 2, "note": "an unasked file", "commit": room.heads[0],
                       "notes": [{ "kind": "unasked", "file": "feature.txt", "says": NOTE }] });
    let kept = room.call(&vale, "finish_task", left);
    assert_eq!(kept["isError"], false, "{}", text(&kept));
    std::fs::write(vale_cwd.join("go"), "").unwrap();
    let fix = common::until(120, "Gathr to give the fix", || {
        let fix = room.task(3);
        (fix["what"] == "Fix the review of #5").then_some(fix)
    });
    assert_eq!(fix["in_passing"], true, "{fix}");
    let review = room.task(2);
    assert_eq!(review["state"], "blocked", "{review}");
    assert_eq!(
        review["notes"],
        json!([{ "kind": "unasked", "file": "feature.txt", "says": NOTE }]),
        "{review}"
    );
    // Looks go on, and none lands the PR: a left review holds it.
    let seen = room.looks();
    common::until(120, "the service to look at #5 again", || {
        (room.looks() >= seen + 3).then_some(())
    });
    assert_eq!(room.main(), room.base, "a left review landed its PR");
    let sql = "SELECT ref FROM event_log WHERE name = 'gathr.review.fix'";
    let rows = common::rows(&data.0, sql);
    let reference = rows[0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let trace = trace.to_string();
    assert!(
        ok && trace.contains("given") && trace.contains("gathr.task.give"),
        "{trace}"
    );

    let done =
        json!({ "how": "done", "task": 2, "commit": room.heads[0], "nothing_unasked": true });
    let landed = room.call(&vale, "finish_task", done);
    let said = text(&landed);
    assert_eq!(landed["isError"], false, "{said}");
    assert!(said.contains("Merged #5"), "{said}");
    assert_eq!(room.main(), room.heads[0], "main isn't the PR's head");
}
