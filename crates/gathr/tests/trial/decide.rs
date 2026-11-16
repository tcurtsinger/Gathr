//! What decides whether a line ships (R6): runs counted in each arm, never a median. A case on two
//! engines is decided on both together and must not be worse on either.

use serde_json::{Value, json};

use super::cases::{Arm, Block, Case, Engine};
use super::results;

/// Runs that end before they say anything of the line: left out of both arms.
const LEFT_OUT: [&str; 3] = ["spoiled", "usage_limit", "signed_out"];
/// A T5 run stopped by its limits says nothing of the question either.
const T5_LEFT_OUT: [&str; 2] = ["timed_out", "over_cap"];

/// Each shipped line's row for the trial's file. Nothing is decided when the model an agent's
/// setting resolved to changed during the trial.
pub fn lines(runs: &[Value], blocks: &[Block], models_same: bool) -> Vec<Value> {
    let case_of = |id: &str| {
        [Case::T1, Case::T2, Case::T5, Case::T8]
            .into_iter()
            .find(|c| c.line() == id)
    };
    let mut rows = Vec::new();
    for (id, text) in gathr::roles::SHIPPED {
        let Some(case) = case_of(id) else { continue };
        let decision = match models_same {
            true => decision(case, runs, blocks),
            false => "undecided",
        };
        let sha = results::line_sha256(text);
        rows.push(json!({ "id": id, "sha256": sha, "case": case.name(), "decision": decision }));
    }
    rows
}

/// The case's runs in `arm`, on `engine` or on both, that count.
fn counted(runs: &[Value], case: Case, arm: Arm, engine: Option<Engine>) -> Vec<&Value> {
    let on = |r: &&Value| engine.is_none_or(|e| r["engine"] == e.name());
    let of = |r: &&Value| r["case"] == case.name() && r["arm"] == arm.name();
    let limited = |o: &&str| case == Case::T5 && T5_LEFT_OUT.contains(o);
    let left_out = |o: &&str| LEFT_OUT.contains(o) || limited(o);
    let kept = |r: &&Value| !r["outcome"].as_str().is_some_and(|o| left_out(&o));
    runs.iter().filter(of).filter(on).filter(kept).collect()
}

fn how_many(runs: &[&Value], holds: impl Fn(&Value) -> bool) -> i64 {
    runs.iter().filter(|r| holds(r)).count() as i64
}

fn decision(case: Case, runs: &[Value], blocks: &[Block]) -> &'static str {
    let planned = |arm: Arm| {
        let of = |b: &&Block| b.arm == arm;
        let seats = blocks.iter().filter(of).flat_map(|b| &b.seats);
        seats.filter(|s| s.case == case).count()
    };
    for arm in [Arm::With, Arm::Without] {
        let (planned, ran) = (planned(arm), counted(runs, case, arm, None).len());
        if planned == 0 || ran * 3 < planned * 2 {
            return "undecided";
        }
    }
    let runs_on = |e: &Engine| {
        blocks
            .iter()
            .flat_map(|b| &b.seats)
            .any(|s| s.case == case && s.engine == *e)
    };
    let engines: Vec<Engine> = [Engine::Claude, Engine::Codex]
        .into_iter()
        .filter(runs_on)
        .collect();
    // How many more runs it holds for with the line than without, on both engines or on one.
    let gain = |engine: Option<Engine>, holds: &dyn Fn(&Value) -> bool| {
        let n = |arm| how_many(&counted(runs, case, arm, engine), holds);
        n(Arm::With) - n(Arm::Without)
    };
    let each = |holds: &dyn Fn(&Value) -> bool, ok: &dyn Fn(i64) -> bool| {
        engines.iter().all(|e| ok(gain(Some(*e), holds)))
    };
    // A run that ran out of time or tokens, or built nothing that works, isn't good.
    let good = |r: &Value| {
        r["outcome"] == "compared" && r["hidden_test"] == true && r["over_built"] == false
    };
    let passed = |r: &Value| r["outcome"] == "passed";
    let asked = |r: &Value| r["named_answered"].as_str().is_some_and(|w| w != "none");
    let choices = |r: &Value| r["with_choices"] == true;
    let kept = match case {
        Case::T1 => gain(None, &good) >= 4 && each(&good, &|g| g >= 0),
        Case::T5 => {
            let any = gain(None, &|r: &Value| r["questions"].as_u64() > Some(0));
            let (asked, choices) = (gain(None, &asked), gain(None, &choices));
            (asked <= -3 || choices >= 3) && asked <= 0 && choices >= 0 && any >= -1
        }
        Case::T8 => gain(None, &passed) >= 4 && each(&passed, &|g| g >= 0),
        Case::T2 => gain(None, &passed) >= 3,
    };
    if kept { "kept" } else { "dropped" }
}
