//! The trial of role text on real engines (0066): each case runs with the lines and without them,
//! block by block, on the room's models and efforts, and what each agent did is counted from
//! Gathr's own records. It never runs by itself: `cargo nextest run` and CI skip ignored tests.
//!
//! `GATHR_TRIAL_DATA` names its data folder, signed in to both engines; `GATHR_TRIAL_COMMIT` the
//! commit it runs at. `GATHR_TRIAL_FIRST=1` makes the first small run instead (one T1 and one T8 on
//! each engine, with the lines only), and `GATHR_TRIAL_STANDIN=1` runs the first round on the replay
//! stand-in engines. It writes `trial/results.json` and `trial/log.txt` in its data folder.

#[path = "trial/block.rs"]
mod block;
#[path = "trial/cases.rs"]
mod cases;
mod common;
#[path = "trial/decide.rs"]
mod decide;
#[path = "trial/file.rs"]
mod file;
#[path = "trial/project.rs"]
mod project;
#[path = "roles/results.rs"]
mod results;
#[path = "trial/score.rs"]
mod score;
#[path = "trial/setup.rs"]
mod setup;
#[path = "trial/standin.rs"]
mod standin;
#[path = "trial/tests.rs"]
mod tests;

use std::path::{Path, PathBuf};

use cases::{Arm, Block, Mode};
use common::TestService;
use serde_json::{Value, json};
use setup::say;

/// The service for a block: its arm's switch (R2), and on the stand-in, the replay engines. The
/// trial's own environment never has the switch, so the with arm's service doesn't either.
fn start(data: &Path, block: &Block, mode: Mode) -> TestService {
    let mut env: Vec<(&str, PathBuf)> = match mode {
        Mode::StandIn => standin::env(data, block.n),
        _ => Vec::new(),
    };
    if block.arm == Arm::Without {
        env.push(("GATHR_TRIAL_WITHOUT", PathBuf::from("1")));
    }
    let env: Vec<(&str, &Path)> = env.iter().map(|(k, v)| (*k, v.as_path())).collect();
    TestService::start_with(data, &env)
}

fn mode() -> Mode {
    let set = |name| std::env::var_os(name).is_some();
    match (set("GATHR_TRIAL_STANDIN"), set("GATHR_TRIAL_FIRST")) {
        (true, _) => Mode::StandIn,
        (_, true) => Mode::First,
        _ => Mode::Full,
    }
}

/// The case runs it will make and the most they can cost, said before any turn runs.
fn say_plan(data: &Path, blocks: &[Block]) {
    let runs: usize = blocks.iter().map(|b| b.seats.len()).sum();
    say(
        data,
        &format!("The trial: {} blocks, {runs} case runs.", blocks.len()),
    );
    for b in blocks {
        let seats: Vec<String> = b
            .seats
            .iter()
            .map(|s| format!("{} {}", s.case.name(), s.engine.name()))
            .collect();
        say(
            data,
            &format!("  block {}, {}: {}", b.n, b.arm.name(), seats.join(", ")),
        );
    }
    let most = cases::most(blocks);
    say(
        data,
        &format!(
            "The most it can cost: {most} input tokens, cache included, as each run stops at its limit and \
        no block starts past {}; a turn running when its limit is reached may end past it.",
            cases::CAP
        ),
    );
}

#[test]
#[ignore = "real engines: Claude runs it by hand"]
fn trial() {
    let data = std::env::var_os("GATHR_TRIAL_DATA").map(PathBuf::from);
    let data = data.expect("GATHR_TRIAL_DATA names the trial's data folder: see docs/testing.md");
    let own = gathr::platform::default_data_dir();
    assert!(
        !setup::same_folder(&data, &own),
        "GATHR_TRIAL_DATA is the person's own Gathr folder: give the trial a spare one"
    );
    let never = |name| std::env::var_os(name).is_none();
    assert!(
        never("GATHR_TEST_KEEP"),
        "unset GATHR_TEST_KEEP: a failing trial would copy its signed-in folder there"
    );
    assert!(
        never("GATHR_TRIAL_WITHOUT"),
        "unset GATHR_TRIAL_WITHOUT: the trial sets it for each block"
    );
    if !cfg!(debug_assertions) {
        panic!("a debug build: a release build ignores the switch, so both arms would be the same");
    }
    let mode = mode();
    let prepared = gathr::data::DataDir::new(&data).db().is_file();
    assert!(
        mode == Mode::StandIn || prepared,
        "{} isn't a prepared data folder",
        data.display()
    );
    let commit = match mode {
        Mode::StandIn => "0000000".to_string(),
        _ => std::env::var("GATHR_TRIAL_COMMIT")
            .expect("GATHR_TRIAL_COMMIT names the commit the trial runs at"),
    };
    std::fs::create_dir_all(setup::dir(&data)).unwrap();
    let blocks = cases::plan(mode);
    say_plan(&data, &blocks);
    let left = setup::clean(&data);
    say(
        &data,
        &format!("Chats an earlier trial left in the list, archived: {left}"),
    );

    let service = start(&data, &blocks[0], mode);
    let ids = setup::prepare(&service, &data, mode == Mode::StandIn);
    let (models, copies) = setup::models(&service, &ids);
    say(
        &data,
        &format!("Models at the start: {models:?}; engine copies {copies}"),
    );
    service.stop();
    let head = json!({ "date": &gathr::ids::utc(gathr::ids::now_us())[..10], "gathr": commit, "copies": copies });

    let (mut runs, mut ended) = (Vec::<Value>::new(), "done");
    for block in &blocks {
        if !cases::may_start(file::spent(&runs)) {
            ended = "cap";
            break;
        }
        say(
            &data,
            &format!(
                "Block {}, {}: {} input tokens spent so far.",
                block.n,
                block.arm.name(),
                file::spent(&runs)
            ),
        );
        let written = setup::write_block(&data, &ids, block);
        let service = start(&data, block, mode);
        let done = block::run(service, &data, &ids, block, &written, mode == Mode::StandIn);
        runs.extend(done.rows);
        // Only the end of the trial decides.
        let lines = decide::lines(&runs, &blocks, false);
        std::fs::write(
            setup::dir(&data).join("results.json"),
            file::file(&head, &runs, lines, ended).to_string(),
        )
        .unwrap();
        if let Some(account) = done.account {
            say(
                &data,
                &format!("The trial ends after block {}: {account}.", block.n),
            );
            ended = account;
            break;
        }
    }

    let service = start(&data, blocks.last().unwrap(), mode);
    let (at_end, _) = setup::models(&service, &ids);
    service.stop();
    let same = at_end == models;
    if !same {
        say(
            &data,
            &format!("The models changed during the trial, to {at_end:?}: it decides nothing."),
        );
    }
    let lines = decide::lines(&runs, &blocks, same);
    let written = file::file(&head, &runs, lines, ended);
    std::fs::write(
        setup::dir(&data).join("results.json"),
        serde_json::to_string_pretty(&written).unwrap(),
    )
    .unwrap();
    say(&data, &format!("Lines: {}", json!(written["lines"])));
    for run in &runs {
        say(&data, &format!("  {run}"));
    }
    assert_eq!(
        results::shape(&written),
        Vec::<String>::new(),
        "the file holds something outside R8's list"
    );
}

/// The trial's block writes, in a process of their own that has the block's arm set (R2). Started
/// by `trial` alone, with `GATHR_TRIAL_BLOCK`: without it, it does nothing.
#[test]
#[ignore = "a block's writes: the trial starts it"]
fn trial_block() {
    let (Some(data), Some(file)) = (
        std::env::var_os("GATHR_TRIAL_DATA"),
        std::env::var_os("GATHR_TRIAL_BLOCK"),
    ) else {
        return;
    };
    let file = PathBuf::from(file);
    let asked: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let written = setup::write(Path::new(&data), &asked);
    std::fs::write(file.with_file_name("block.out.json"), written.to_string()).unwrap();
}
