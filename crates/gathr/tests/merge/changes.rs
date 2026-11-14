//! 0054, scenario 10h through the service: Gathr measures a PR when it would land it, one past its
//! project's line waits for the person's answer, and every change that merges is recorded.
use super::*;
use lands::looking;

const GO_ON: &str = "Let it go on";
const SEND_BACK: &str = "Send it back";

/// `n` lines of made-up code.
fn lines(n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| format!("let x{i} = {i};\n"))
        .collect::<String>()
        .into_bytes()
}

impl Room {
    /// PR `n`'s branch (0 is #5, 1 is #6) moved to one commit on `from` that adds `files`, as its
    /// author force-pushes it: GitHub shows the new head, and the project's checks passed there.
    fn push_head(&self, n: usize, from: &str, files: &[(&str, Vec<u8>)]) -> String {
        let branch = ["room/five", "room/six"][n];
        git(&self.other, &["checkout", "-q", "-B", branch, from]);
        let head = self.commit(files, branch);
        git(&self.other, &["push", "-q", "--force", "origin", branch]);
        git(&self.other, &["checkout", "-q", "main"]);
        self.checks_passed(&head);
        let mut all = self.answers();
        all[n]["merged_if"]["head"] = json!(head);
        put_answers(&self.data, &all);
        self.view(n, |view| view["headRefOid"] = json!(head));
        head
    }

    /// A commit on main that adds `files`, pushed as another PR's merge would be: main's commit.
    fn on_main(&self, files: &[(&str, Vec<u8>)]) -> String {
        git(&self.other, &["checkout", "-q", "main"]);
        let commit = self.commit(files, "on main");
        git(&self.other, &["push", "-q", "origin", "main"]);
        commit
    }

    fn commit(&self, files: &[(&str, Vec<u8>)], message: &str) -> String {
        for (path, body) in files {
            let path = self.other.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        git(&self.other, &["add", "."]);
        git(&self.other, &["commit", "-q", "-m", message]);
        git(&self.other, &["rev-parse", "HEAD"]).trim().to_string()
    }

    /// Gathr's newest open question to the person, once it's there.
    fn question(&self, after: i64) -> Value {
        common::until(60, "Gathr's question to the person", || {
            let open = self.service.call("GET", "/api/person-tasks").json();
            let open = open["tasks"].as_array()?.iter();
            let mut gathrs =
                open.filter(|t| t["giver"].is_null() && t["number"].as_i64() > Some(after));
            gathrs.next_back().cloned()
        })
    }

    /// The person's answer, as the window sends it.
    fn person_answers(&self, answer: Value) -> Value {
        let body = json!({ "answers": [answer] });
        let sent = self.service.send("POST", "/api/person-tasks/answer", body);
        assert_eq!(sent.status, 200, "{}", sent.body);
        sent.json()
    }

    /// PR #5's row of what Gathr measured and recorded, or null.
    fn change(&self) -> Value {
        let rows = common::rows(&self.data, "SELECT * FROM changes WHERE pr = 5");
        rows.into_iter().next().unwrap_or(Value::Null)
    }

    fn events(&self, name: &str) -> i64 {
        let sql = format!("SELECT count(*) AS n FROM event_log WHERE name = '{name}'");
        common::rows(&self.data, &sql)[0]["n"].as_i64().unwrap()
    }

    /// The reviewer reports its review `task` done on `commit`: what Gathr answered.
    fn report(&self, vale: &str, task: i64, commit: &str) -> String {
        let done =
            json!({ "how": "done", "task": task, "commit": commit, "nothing_unasked": true });
        let finished = self.call(vale, "finish_task", done);
        let said = text(&finished);
        assert_eq!(finished["isError"], false, "{said}");
        said
    }

    /// Lead's turn, Ann's work on #5 given in it, and Vale's review of #5: their tokens.
    fn built_and_reviewed(&mut self) -> (String, String, String) {
        let lead = self.lead_turn();
        let work = json!({ "to": "Ann", "what": "Add the feature", "pr": 5 });
        let given = text(&self.call(&lead, "give_task", work));
        assert!(given.starts_with("Task #1 for Ann"), "{given}");
        let ann = self.token();
        let vale = self.reviews(&lead, &[5]);
        (lead, ann, vale)
    }
}

/// The `pr` line of what the chat waits on: its words and who moves it.
fn pr_line(room: &Room) -> (Value, Value) {
    let waiting = common::waiting(&room.service, &room.chat);
    let lines = waiting["lines"].as_array().unwrap();
    let line = lines.iter().find(|l| l["kind"] == "pr");
    let line = line.unwrap_or_else(|| panic!("no pr line: {waiting}"));
    (line["what"].clone(), line["moves"].clone())
}

#[test]
fn landing_measures_a_head_once_and_counts_only_code() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    let main = room.on_main(&[(".gitattributes", b"gen/** linguist-generated\n".to_vec())]);
    let png = vec![0x89, b'P', b'N', b'G', 0, 0, 0, 13, 1, 2, 3];
    let files = [
        ("src/big.rs", lines(2000)),
        ("Cargo.lock", lines(9000)),
        ("logo.png", png),
        ("gen/api.rs", lines(500)),
    ];
    let head = room.push_head(0, &main, &files);
    room.run(&head, "running");
    room.review(1, 5, Some(&head));
    room.looked();
    room.looked();
    assert_eq!(room.main(), main, "landed while its checks ran");
    let change = room.change();
    assert_eq!(
        (&change["counted"], &change["line"], &change["question"]),
        (&json!(2000), &json!(3000), &Value::Null),
        "{change}"
    );
    assert_eq!(change["top"], json!(r#"[["src/big.rs",2000]]"#), "{change}");
    assert_eq!(
        room.events("gathr.change.measured"),
        1,
        "measured again at the same head and line"
    );
    room.run(&head, "passed");
    room.landed(&head);
}

#[test]
fn a_change_past_the_line_waits_for_the_person_and_doesnt_land() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let head = room.push_head(0, &room.base.clone(), &[("src/big.rs", lines(3200))]);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let said = room.report(&vale, 1, &head);
    let q = room.question(0);
    let n = q["number"].as_i64().unwrap();
    assert!(
        said.contains(&format!("waits on the person's answer to #{n}")),
        "{said}"
    );
    assert_eq!(
        q["what"], "#5 adds 3,200 lines, over this project's line of 3,000: let it go on?",
        "{q}"
    );
    let labels: Vec<&Value> = q["choices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["label"])
        .collect();
    assert_eq!(labels, [GO_ON, SEND_BACK], "{q}");
    assert_eq!(room.main(), room.base, "landed past the line");
    let expected = format!("#5 to land: waits on your answer to #{n}");
    assert_eq!(pr_line(&room), (json!(expected), json!("you")));
    let merge = room.call(&vale, "merge", json!({ "pr": 5 }));
    room.refused(&merge, &format!("#5 waits on the person's answer to #{n}"));
    assert_eq!(room.main(), room.base, "merged past the line");
}

#[test]
fn let_it_go_on_lands_it_and_records_the_change() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let head = room.push_head(0, &room.base.clone(), &[("src/big.rs", lines(3200))]);
    let (_, ann, vale) = room.built_and_reviewed();
    room.report(&vale, 2, &head);
    let n = room.question(0)["number"].as_i64().unwrap();
    let by_ann = room.call(&ann, "answer_person", json!({ "task": n, "pick": [GO_ON] }));
    let said = text(&by_ann);
    assert!(
        by_ann["isError"] == true && said.contains("only the person answers it"),
        "{said}"
    );
    let in_words = room.person_answers(json!({ "task": n, "text": "fine" }));
    let why = format!("Answer #{n} with one of its choices");
    assert_eq!(in_words["refused"][0]["why"], why.as_str(), "{in_words}");

    let answered = room.person_answers(json!({ "task": n, "pick": [GO_ON] }));
    assert_eq!(answered["refused"], json!([]), "{answered}");
    // The PR looks run every 2 minutes here: within one, the answer itself had Gathr look.
    common::until(60, "the PR to land on the answer", || {
        (room.main() == head).then_some(())
    });
    let change = common::until(30, "the change's record", || {
        let change = room.change();
        change["landed_ts"].is_i64().then_some(change)
    });
    let kept = |key: &str| change[key].clone();
    assert_eq!(
        [
            kept("counted"),
            kept("let_through"),
            kept("by_gathr"),
            kept("unasked")
        ],
        [json!(3200), json!(3200), json!(1), json!(0)],
        "{change}"
    );
    assert_eq!(
        [
            kept("builder_id"),
            kept("reviewer_id"),
            kept("first_reviewed")
        ],
        [json!(room.ann), json!(room.vale), json!(head)],
        "{change}"
    );
    assert!(
        change["builder_engine"].is_string() && change["reviewer_engine"].is_string(),
        "{change}"
    );
}

#[test]
fn a_force_pushed_smaller_head_withdraws_and_lands() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let base = room.base.clone();
    let big = room.push_head(0, &base, &[("src/big.rs", lines(3200))]);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.report(&vale, 1, &big);
    let n = room.question(0)["number"].as_i64().unwrap();
    let small = room.push_head(0, &base, &[("src/big.rs", lines(500))]);
    let said = room.report(&vale, 1, &small);
    assert!(said.contains("Merged #5"), "{said}");
    assert_eq!(room.main(), small);
    let q = room.task(n);
    assert_eq!(
        (&q["state"], &q["reason"]),
        (
            &json!("cancelled"),
            &json!("#5 is now +500, under this project's line of 3,000")
        ),
        "{q}"
    );
}

#[test]
fn a_bigger_head_before_the_answer_is_asked_again() {
    for (added, lands) in [(6201, false), (6200, true)] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = room(&data.0, &repos.0, false);
        let base = room.base.clone();
        let first = room.push_head(0, &base, &[("src/big.rs", lines(3200))]);
        let lead = room.lead_turn();
        let vale = room.reviews(&lead, &[5]);
        room.report(&vale, 1, &first);
        let n = room.question(0)["number"].as_i64().unwrap();
        let bigger = room.push_head(0, &base, &[("src/big.rs", lines(added))]);
        let said = room.report(&vale, 1, &bigger);
        let rests = format!("waits on the person's answer to #{n}");
        assert!(said.contains(&rests), "{added}: {said}");
        room.person_answers(json!({ "task": n, "pick": [GO_ON] }));
        if lands {
            common::until(60, "the PR to land on the answer", || {
                (room.main() == bigger).then_some(())
            });
            continue;
        }
        let q2 = room.question(n);
        assert_eq!(
            q2["what"], "#5 adds 6,201 lines, over this project's line of 3,000: let it go on?",
            "{q2}"
        );
        let n2 = q2["number"].as_i64().unwrap();
        let sql = format!("SELECT saw FROM landings WHERE pr = 5 AND head = '{bigger}'");
        common::until(60, "Gathr's look after the answer", || {
            let saw = common::rows(&room.data, &sql);
            let saw = saw.first()?["saw"].as_str()?.to_string();
            saw.contains(&format!("#{n2}")).then_some(())
        });
        assert_eq!(room.main(), base, "landed past what was let through");
    }
}

/// The waking fixture, where each engine ends a turn on its own gate: the lead's giving turn has
/// ended, as it has long before a PR lands, so its hear-back starts a turn.
#[test]
fn send_it_back_blocks_the_work_and_that_head_never_lands() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = looking(&data.0, &repos.0, false);
    let head = room.push_head(0, &room.base.clone(), &[("src/big.rs", lines(3200))]);
    let lead = room.lead_turn();
    let lead_cwd = PathBuf::from(nth(&data.0.join("env.jsonl"), 0)["cwd"].as_str().unwrap());
    std::fs::write(lead_cwd.join("go"), "").unwrap();
    let gave = room
        .service
        .wait_for(&room.chat, 20, "the giving turn to end", |h| {
            h["turns"][0]["status"] == "done"
        });
    let gave = gave["turns"][0]["id"].as_str().unwrap().to_string();
    let work = "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, pr, given_in,
                                   created_ts, updated_ts)
                VALUES (1, ?1, ?2, ?3, 'work', 'Add the feature', 'working', 5, ?4, ?5, ?5)";
    // Given now: a task that moved long ago is stale, and its giver is told (F11).
    let now = gathr::ids::now_us();
    let row = rusqlite::params![room.chat, room.lead, room.ann, gave, now];
    room.db().execute(work, row).unwrap();
    room.review(2, 5, Some(&head));
    let n = room.question(0)["number"].as_i64().unwrap();
    let answered = room.person_answers(json!({ "task": n, "pick": [SEND_BACK] }));
    assert_eq!(answered["refused"], json!([]), "{answered}");
    let why = "The person sent #5 back as bigger than asked: +3,200 lines; this project's line is 3,000. \
               Narrow #1's ask and restart it, or cancel it.";
    let work = room.task(1);
    assert_eq!(
        (&work["state"], &work["reason"]),
        (&json!("blocked"), &json!(why)),
        "{work}"
    );
    let turns_of = |room: &Room, agent: &str| -> Vec<Value> {
        let chat = room.chat();
        let turns = chat["turns"].as_array().unwrap().iter();
        turns.filter(|t| t["agent"] == agent).cloned().collect()
    };
    let heard = common::until(30, "Lead to hear back", || {
        turns_of(&room, &room.lead).get(1).cloned()
    });
    assert_eq!(heard["answers"], gave.as_str(), "{heard}");
    assert_eq!(
        turns_of(&room, &room.lead).len(),
        2,
        "Lead didn't hear back once"
    );
    let expected = "#5 to land: you sent it back; waits on a new head";
    assert_eq!(pr_line(&room), (json!(expected), json!("others")));

    // A new review at the head sent back: looks go on, and none lands it or wakes anyone.
    room.review(n + 1, 5, Some(&head));
    room.looked();
    room.looked();
    assert_eq!(room.main(), room.base, "a head sent back landed");
    let sql = format!("SELECT saw FROM landings WHERE pr = 5 AND head = '{head}'");
    let saw = common::rows(&room.data, &sql);
    let short = &head[..7];
    assert_eq!(
        saw[0]["saw"],
        format!("was sent back by the person at {short}"),
        "{saw:?}"
    );
    assert_eq!(
        room.events("gathr.quiet.woke"),
        0,
        "a send-back woke someone"
    );

    let restarted = room.call(&lead, "restart_task", json!({ "task": 1 }));
    assert_eq!(restarted["isError"], false, "{}", text(&restarted));
    let carried = format!("Continue from here: {why}");
    common::until(30, "Ann's turn to carry why", || {
        let turn = turns_of(&room, &room.ann).first()?["id"].clone();
        let chat = room.chat();
        let mut messages = chat["messages"]
            .as_array()?
            .iter()
            .filter(|m| m["turn"] == turn);
        messages.find(|m| m["body"].as_str().is_some_and(|b| b.contains(&carried)))?;
        Some(())
    });
}

#[test]
fn a_pr_merged_outside_gathr_cancels_its_question_and_is_recorded() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    let head = room.push_head(0, &room.base.clone(), &[("src/big.rs", lines(3200))]);
    room.review(1, 5, Some(&head));
    let n = room.question(0)["number"].as_i64().unwrap();
    // Someone merges it by hand: GitHub shows it merged once its head is in main.
    git(
        &room.other,
        &["push", "-q", "origin", &format!("{head}:main")],
    );
    let change = common::until(120, "the change's record", || {
        let change = room.change();
        change["landed_ts"].is_i64().then_some(change)
    });
    assert_eq!(
        (&change["by_gathr"], &change["reviewer_id"]),
        (&json!(0), &json!(room.vale)),
        "{change}"
    );
    let q = room.task(n);
    assert_eq!(
        (&q["state"], &q["reason"]),
        (&json!("cancelled"), &json!("#5 merged")),
        "{q}"
    );
    assert_eq!(room.events("gathr.change.merged_unasked"), 1);
}

#[test]
fn an_unconfirmed_push_seen_merged_later_counts_as_gathrs() {
    for by_tool in [false, true] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = match by_tool {
            false => looking(&data.0, &repos.0, false),
            true => room(&data.0, &repos.0, false),
        };
        // GitHub goes on showing the PR open after Gathr's push.
        let shown = room.answers();
        let mut open = shown.clone();
        open[0].as_object_mut().unwrap().remove("merged_if");
        put_answers(&room.data, &open);
        if by_tool {
            let lead = room.lead_turn();
            let vale = room.reviews(&lead, &[5]);
            room.review_done(1, &room.heads[0]);
            let merged = room.call(&vale, "merge", json!({ "pr": 5 }));
            assert!(
                text(&merged).contains("doesn't show #5 merged"),
                "{}",
                text(&merged)
            );
        } else {
            room.review(1, 5, Some(&room.heads[0]));
            let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.merge.done'
                       AND error_type = 'unconfirmed'";
            common::until(120, "Gathr's push GitHub doesn't confirm", || {
                (common::rows(&room.data, sql)[0]["n"] == 1).then_some(())
            });
        }
        put_answers(&room.data, &shown);
        if by_tool {
            // Its PR looks run every 2 minutes: a service that looks often sees the merge sooner.
            // Its held turns are let go first, or the stop waits 10 s on them; the new one's at the end.
            room._release = Release(room.data.join("go"), true);
            let _ = room.service.call("POST", "/api/shutdown");
            room.service.child.wait().unwrap();
            room.service = service(&room.data, true);
        }
        let change = common::until(120, "the change's record", || {
            let change = room.change();
            change["landed_ts"].is_i64().then_some(change)
        });
        assert_eq!(change["by_gathr"], 1, "by the tool: {by_tool}: {change}");
    }
}

#[test]
fn the_line_follows_the_largest_recent_change() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    // Main's last 11 changes: the largest adds 9,000 lines.
    for i in 0..8 {
        room.on_main(&[(&format!("src/f{i}.rs"), lines(10))]);
    }
    let main = room.on_main(&[("src/big.rs", lines(9000))]);
    let over = room.push_head(1, &main, &[("src/more.rs", lines(9001))]);
    let at = room.push_head(0, &main, &[("src/more.rs", lines(9000))]);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[6, 5]);
    let said = room.report(&vale, 1, &over);
    let q = room.question(0);
    assert_eq!(
        q["what"], "#6 adds 9,001 lines, over this project's line of 9,000: let it go on?",
        "{said}"
    );
    let said = room.report(&vale, 2, &at);
    assert!(said.contains("Merged #5"), "{said}");
}

/// The count reads the diff the sign-off read: one git can't make (a head with no history in
/// common with main) stops the look as it did before, writes no row, and is found from its
/// reference.
#[test]
fn a_diff_that_fails_at_landing_writes_no_row_and_is_found_from_its_reference() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    git(&room.other, &["checkout", "-q", "--orphan", "room/lone"]);
    let head = room.commit(&[("lone.txt", b"alone\n".to_vec())], "lone");
    git(&room.other, &["push", "-q", "origin", "room/lone"]);
    git(&room.other, &["checkout", "-q", "-f", "main"]);
    room.checks_passed(&head);
    room.view(0, |view| view["headRefOid"] = json!(head));
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let said = room.report(&vale, 1, &head);
    assert!(said.contains("couldn't look at #5"), "{said}");
    let rows = common::rows(&room.data, "SELECT count(*) AS n FROM changes");
    assert_eq!(rows[0]["n"], 0, "a row for a change that wasn't measured");
    let sql = "SELECT ref FROM event_log WHERE name = 'gathr.land.look' AND status = 'error'";
    let found = common::rows(&room.data, sql);
    let reference = found[0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&room.data, &["diag", "trace", reference]);
    assert!(
        ok && trace.to_string().contains("git diff failed"),
        "{trace}"
    );
    assert_eq!(room.main(), room.base);
}
