//! 0064, scenarios 2h and 2i through the service: when a building task reports done naming its PR,
//! Gathr counts the PR before any review is given. Past the task's line it asks the person once,
//! and the task waits; the answer gives the review or sends the task back.
use super::*;
use changes::lines;

const GO_ON: &str = "Let it go on";
const SEND_BACK: &str = "Send it back";
/// The engines in the order they start: Lead's, Ann's, Vale's.
const LEAD: usize = 0;
const ANN: usize = 1;

/// The merge room, where each engine's turn holds on a gate of its own (`go` in its folder, used up
/// by the turn it lets go), so a test ends one agent's turn while the others hold; with `more` of
/// the environment.
fn gated(data: &Path, repos: &Path, more: &[(&str, &Path)]) -> Room {
    let mut room = room_replay(data, repos, false, true);
    let _ = room.service.call("POST", "/api/shutdown");
    room.service.child.wait().unwrap();
    let mut env: Vec<(&str, &Path)> = vec![("GATHR_REPLAY_HOLD", Path::new("go"))];
    env.extend_from_slice(more);
    room.service = service_with(data, false, &env);
    room
}

impl Room {
    /// Lead's turn sizes the ask a repair and gives Ann #1 with no PR, with `lines` in its brief
    /// when given; Ann's turn on it runs. Lead's and Ann's tokens.
    fn given(&mut self, lines: Option<i64>) -> (String, String) {
        let lead = self.lead_turn();
        common::fix_it(self.service.info.port, &lead);
        let mut work = json!({ "to": "Ann", "what": "Fix the export" });
        if let Some(n) = lines {
            work["size"] = json!("src/export.rs");
            work["lines"] = json!(n);
        }
        let given = text(&self.call(&lead, "give_task", common::briefed(work)));
        assert!(given.starts_with("Task #1 for Ann"), "{given}");
        let ann = self.token();
        self.running(1);
        (lead, ann)
    }

    /// The turn running on `task`, once one is.
    fn running(&self, task: i64) -> String {
        let sql = "SELECT id FROM turns WHERE task = ?1 AND status = 'running' ORDER BY rowid DESC";
        common::until(30, &format!("a turn on #{task} to run"), || {
            self.db().query_row(sql, [task], |r| r.get(0)).ok()
        })
    }

    /// Ann reports `task` done naming PR #5: Gathr's answer.
    fn done(&self, ann: &str, task: i64) -> String {
        let report = json!({ "how": "done", "task": task, "pr": 5 });
        let finished = self.call(ann, "finish_task", report);
        let said = text(&finished);
        assert_eq!(finished["isError"], false, "{said}");
        said
    }

    /// The running turn of engine `n` (of `agent`) goes on to its end.
    fn release(&self, n: usize, agent: &str) {
        let running = "SELECT id FROM turns WHERE agent_id = ?1 AND status = 'running'";
        let turn: String = common::until(30, "a turn to let go", || {
            self.db().query_row(running, [agent], |r| r.get(0)).ok()
        });
        let cwd = PathBuf::from(
            nth(&self.data.join("env.jsonl"), n)["cwd"]
                .as_str()
                .unwrap(),
        );
        std::fs::write(cwd.join("go"), "").unwrap();
        let status = "SELECT status FROM turns WHERE id = ?1";
        common::until(30, "the turn let go to end", || {
            let now: String = self.db().query_row(status, [&turn], |r| r.get(0)).ok()?;
            (now != "running").then_some(())
        });
    }

    /// The person makes the chat's ask `size`.
    fn resized(&self, size: &str) {
        let listed = self
            .service
            .call("GET", &format!("/api/chats/{}/tasks", self.chat))
            .json();
        let ask = listed["asks"][0]["id"].as_str().unwrap().to_string();
        let path = format!("/api/asks/{ask}/size");
        let sized = self.service.send("POST", &path, json!({ "size": size }));
        assert_eq!(sized.status, 200, "{}", sized.body);
    }

    /// The chat's tasks of `kind`.
    fn of_kind(&self, kind: &str) -> Vec<Value> {
        let path = format!("/api/chats/{}/tasks", self.chat);
        let listed = self.service.call("GET", &path).json();
        let all = listed["tasks"].as_array().unwrap().iter();
        all.filter(|t| t["kind"] == kind).cloned().collect()
    }

    /// Gathr's questions to the person, open.
    fn open_questions(&self) -> Vec<Value> {
        let open = self.service.call("GET", "/api/person-tasks").json();
        let open = open["tasks"].as_array().unwrap().iter();
        open.filter(|t| t["giver"].is_null()).cloned().collect()
    }

    /// Each count at a done report: its reference, error, outcome and why.
    fn counts(&self) -> Vec<Value> {
        let sql =
            "SELECT ref, error_type, attrs ->> '$.outcome' AS outcome, attrs ->> '$.why' AS why
                   FROM event_log WHERE name = 'gathr.change.report' ORDER BY ts";
        common::rows(&self.data, sql)
    }

    fn turns_of(&self, agent: &str) -> Vec<Value> {
        let chat = self.chat();
        let turns = chat["turns"].as_array().unwrap().iter();
        turns.filter(|t| t["agent"] == agent).cloned().collect()
    }

    /// `agent`'s turn answering `turn`: a hear-back on it, once there.
    fn heard(&self, agent: &str, turn: &Value) -> Value {
        common::until(30, "a hear-back", || {
            self.turns_of(agent)
                .into_iter()
                .find(|t| t["answers"] == *turn)
        })
    }

    /// A second work task of Ann's on the same ask, #50, as if given and at work.
    fn second_task(&self) {
        let sql = "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, ask_id, created_ts, updated_ts)
                   VALUES (50, ?1, ?2, ?3, 'work', 'Add the rest', 'working',
                           (SELECT ask_id FROM tasks WHERE number = 1), ?4, ?4)";
        let row = rusqlite::params![self.chat, self.lead, self.ann, gathr::ids::now_us()];
        self.db().execute(sql, row).unwrap();
    }

    fn state_of(&self, task: i64) -> (Value, Value) {
        let t = self.task(task);
        (t["state"].clone(), t["reason"].clone())
    }

    fn needs(&self) -> Value {
        let chats = self.service.call("GET", "/api/chats").json();
        let chat = chats
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == *self.chat);
        chat.unwrap()["needs"].clone()
    }
}

/// PR #5's head adds 1,240 lines; Ann reports #1, on a repair, done in its turn: Gathr asks. Lead's
/// and Ann's tokens, the question's number and the head.
fn asked(room: &mut Room) -> (String, String, i64, String) {
    let head = room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(1240))]);
    let (lead, ann) = room.given(None);
    let said = room.done(&ann, 1);
    let n = room.question(0)["number"].as_i64().unwrap();
    let asked =
        format!("Gathr asked the person about #5's size (#{n}). Its review waits for the answer.");
    assert!(said.contains(&asked), "{said}");
    (lead, ann, n, head)
}

#[test]
fn a_fix_past_its_line_at_its_done_report_asks_once_and_gives_no_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let (_, ann, n, _) = asked(&mut room);
    let turn = room.running(1);
    let q = room.question(0);
    assert_eq!(
        (&q["giver"], &q["given_in"]),
        (&Value::Null, &json!(turn)),
        "{q}"
    );
    assert_eq!(
        q["what"], "#5 adds 1,240 lines, over a Fix's line of 1,000: let it go on?",
        "{q}"
    );
    let why = q["why"].as_str().unwrap();
    assert_eq!(
        why.lines().next(),
        Some("#5 adds 1,240 lines and removes 0. A Fix's line is 1,000, the least it can be."),
        "{why}"
    );
    let choices: Vec<(&Value, &Value)> = (q["choices"].as_array().unwrap().iter())
        .map(|c| (&c["label"], &c["detail"]))
        .collect();
    let sent_back = json!("#1 is blocked, and Lead is told.");
    assert_eq!(
        choices,
        [
            (&json!(GO_ON), &json!("Its review starts.")),
            (&json!(SEND_BACK), &sent_back)
        ]
    );
    assert_eq!(
        room.of_kind("review"),
        Vec::<Value>::new(),
        "a review given"
    );

    room.release(LEAD, &room.lead.clone());
    room.release(ANN, &room.ann.clone());
    let held = format!("done, waiting on #{n} (the person), #5 to merge");
    assert_eq!(room.state_of(1), (json!("held"), json!(held)));
    assert_eq!(room.turns_of(&room.lead).len(), 1, "Lead was woken");
    // Reported again while asked, by Ann or by a second task naming #5 first: asked once.
    room.done(&ann, 1);
    room.second_task();
    room.done(&ann, 50);
    assert_eq!(room.open_questions().len(), 1);
    assert_eq!(room.events("gathr.change.asked"), 1);
    assert_eq!(
        room.of_kind("review"),
        Vec::<Value>::new(),
        "a review given"
    );
    let merge = json!("done, waiting on #5 to merge");
    assert_eq!(
        room.state_of(50),
        (json!("held"), merge),
        "held by #1's question"
    );
}

#[test]
fn send_it_back_blocks_the_task_and_its_smaller_head_gets_its_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let (lead, ann, n, head) = asked(&mut room);
    room.release(LEAD, &room.lead.clone());
    room.release(ANN, &room.ann.clone());
    let gave = room.turns_of(&room.lead)[0]["id"].clone();
    room.person_answers(json!({ "task": n, "pick": [SEND_BACK] }));
    let why = "The person sent #5 back as bigger than asked: +1,240 lines; a Fix's line is 1,000. \
               Narrow #1's ask and restart it, or cancel it.";
    assert_eq!(room.state_of(1), (json!("blocked"), json!(why)));
    room.heard(&room.lead, &gave);

    let restarted = room.call(&lead, "restart_task", json!({ "task": 1 }));
    assert_eq!(restarted["isError"], false, "{}", text(&restarted));
    room.running(1);
    let said = room.done(&ann, 1);
    let rested = format!(
        "#5 was sent back by the person at {}. Push a smaller change, then report #1 done.",
        &head[..7]
    );
    assert!(said.contains(&rested), "{said}");
    room.release(ANN, &room.ann.clone());
    assert_eq!(room.state_of(1), (json!("blocked"), json!(rested)));
    assert_eq!(
        room.of_kind("review"),
        Vec::<Value>::new(),
        "a review given"
    );

    room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(300))]);
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    assert_eq!(room.open_questions(), Vec::<Value>::new());
    let given = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.review.give'
                 AND attrs ->> '$.\"gathr.review.outcome\"' = 'given'";
    assert_eq!(common::rows(&room.data, given)[0]["n"], 1);
}

#[test]
fn let_it_go_on_gives_the_review_and_landing_doesnt_ask_again() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let head = room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(500))]);
    let (_, ann) = room.given(Some(100));
    room.resized("new");
    let said = room.done(&ann, 1);
    let q = room.question(0);
    let n = q["number"].as_i64().unwrap();
    assert!(said.contains(&format!("#5's size (#{n})")), "{said}");
    assert_eq!(
        q["what"], "#5 adds 500 lines, over its part's line of 400: let it go on?",
        "{q}"
    );
    let why = q["why"].as_str().unwrap();
    assert_eq!(
        why.lines().next(),
        Some("#5 adds 500 lines and removes 0. Its part said 100 lines; its line is 400."),
        "{why}"
    );
    room.release(ANN, &room.ann.clone());

    room.person_answers(json!({ "task": n, "pick": [GO_ON] }));
    let merge = json!("done, waiting on #5 to merge");
    assert_eq!(room.state_of(1), (json!("held"), merge));
    let reviews = room.of_kind("review");
    assert_eq!(reviews.len(), 1, "{reviews:?}");
    let review = reviews[0]["number"].as_i64().unwrap();
    let vale = room.token();
    room.running(review);
    room.report(&vale, review, &head);
    room.landed(&head);
    let change = room.change();
    assert_eq!(
        (&change["let_through"], &change["line_of"]),
        (&json!(500), &json!("part")),
        "{change}"
    );
    assert_eq!(room.events("gathr.change.asked"), 1, "asked again");
}

#[test]
fn under_its_line_or_with_no_line_the_review_is_given_as_today() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(300))]);
    let (_, ann) = room.given(None);
    let turn = room.running(1);
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    let counts = room.counts();
    assert_eq!(counts.len(), 1, "{counts:?}");
    assert_eq!(
        (&counts[0]["outcome"], &counts[0]["error_type"]),
        (&json!("under"), &Value::Null)
    );
    let review = &room.of_kind("review")[0];
    assert_eq!(review["given_in"], json!(turn), "{review}");

    // An ask made new after the give, with no size given: the task has no line.
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(1240))]);
    let (_, ann) = room.given(None);
    room.resized("new");
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    assert_eq!(room.counts(), Vec::<Value>::new(), "counted");
}

#[test]
fn a_count_that_fails_gives_the_review_and_is_found_from_its_reference() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(1240))]);
    let mut answers = room.answers();
    answers.as_array_mut().unwrap().remove(0);
    put_answers(&room.data, &answers);
    let (_, ann) = room.given(None);
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    assert_eq!(room.task(1)["report"]["how"], "done");
    let counts = room.counts();
    assert_eq!(counts.len(), 1, "{counts:?}");
    assert_eq!(counts[0]["error_type"], "github", "{counts:?}");
    assert!(
        counts[0]["why"].as_str().is_some_and(|w| w.contains("#5")),
        "{counts:?}"
    );
    let (ok, failures) = gathr(&room.data, &["diag", "failures", "--since", "1h"]);
    assert!(
        ok && failures.to_string().contains("gathr.change.report"),
        "{failures}"
    );
    let reference = counts[0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&room.data, &["diag", "trace", reference]);
    let trace = trace.to_string();
    for step in ["finish_task", "gathr.change.report", "gathr.review.give"] {
        assert!(ok && trace.contains(step), "no {step}: {trace}");
    }
}

#[test]
fn a_count_past_its_time_gives_the_review_as_today() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let none = [("GATHR_REPORT_COUNT_MS", Path::new("0"))];
    let mut room = gated(&data.0, &repos.0, &none);
    room.push_head(0, &room.base.clone(), &[("src/export.rs", lines(1240))]);
    let (_, ann) = room.given(None);
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    let counts = room.counts();
    assert_eq!(counts.len(), 1, "{counts:?}");
    assert_eq!(counts[0]["error_type"], "timeout", "{counts:?}");
    assert_eq!(room.open_questions(), Vec::<Value>::new(), "asked");
}

#[test]
fn cancelling_the_question_or_the_task_ends_it() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let (_, ann, n, _) = asked(&mut room);
    room.release(LEAD, &room.lead.clone());
    room.release(ANN, &room.ann.clone());
    let gave = room.turns_of(&room.lead)[0]["id"].clone();
    let cancel = format!("/api/tasks/{n}/cancel");
    let cancelled = room.service.send("POST", &cancel, json!({}));
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let why = format!(
        "The person cancelled #{n}, Gathr's question about #5's size, so #5 doesn't land at this head. \
         Narrow #1's ask and restart it, or cancel it."
    );
    assert_eq!(room.state_of(1), (json!("blocked"), json!(why)));
    room.heard(&room.lead, &gave);
    let picked = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.change.answered'
                  AND attrs ->> '$.pick' = 'cancelled'";
    assert_eq!(common::rows(&room.data, picked)[0]["n"], 1);
    room.done(&ann, 1);
    let rested = format!(
        "#5 waits on a new head: the person cancelled #{n}. Push a smaller change, then report #1 done."
    );
    assert_eq!(room.state_of(1), (json!("blocked"), json!(rested)));
    assert_eq!(
        room.of_kind("review"),
        Vec::<Value>::new(),
        "a review given"
    );

    // The task cancelled while its question is open: the question goes with it.
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let (lead, _, n, _) = asked(&mut room);
    room.release(LEAD, &room.lead.clone());
    room.release(ANN, &room.ann.clone());
    assert_eq!(room.needs(), true);
    let cancelled = room.call(&lead, "cancel_task", json!({ "task": 1 }));
    assert_eq!(cancelled["isError"], false, "{}", text(&cancelled));
    assert_eq!(
        room.state_of(n),
        (json!("cancelled"), json!("#1 was cancelled"))
    );
    assert_eq!(room.needs(), false, "the chat still needs the person");
}

#[test]
fn landing_measures_against_the_tasks_line_once_the_review_is_in() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = gated(&data.0, &repos.0, &[]);
    let base = room.base.clone();
    room.push_head(0, &base, &[("src/export.rs", lines(300))]);
    let (_, ann) = room.given(None);
    let said = room.done(&ann, 1);
    assert!(said.contains("to review #5"), "{said}");
    room.release(ANN, &room.ann.clone());
    let review = room.of_kind("review")[0]["number"].as_i64().unwrap();
    let vale = room.token();
    room.running(review);
    let head = room.push_head(0, &base, &[("src/export.rs", lines(1240))]);
    let said = room.report(&vale, review, &head);
    let q = room.question(0);
    let n = q["number"].as_i64().unwrap();
    assert!(
        said.contains(&format!("waits on the person's answer to #{n}")),
        "{said}"
    );
    assert_eq!(
        q["what"], "#5 adds 1,240 lines, over a Fix's line of 1,000: let it go on?",
        "{q}"
    );
    let go_on = json!("It lands once its review and checks pass.");
    assert_eq!(q["choices"][0]["detail"], go_on, "{q}");
    assert_eq!(room.main(), base, "landed past its line");
    // A second task reporting #5 done once its review is given: nothing is counted.
    room.second_task();
    room.done(&ann, 50);
    assert_eq!(room.counts().len(), 1, "counted again");
}
