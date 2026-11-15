//! Samples of the words a quiet chat wakes its lead with, and a silent turn is told to it with
//! (Stage 3, part A), each with the kind and version its turn records.

use super::*;
use gathr::words;

#[test]
fn a_wake_and_a_silent_turns_telling_are_the_kept_samples() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    idle(&mut conn, "ann");
    let woke = check(&mut conn, &service(), gathr::ids::now_us());
    samples::same("wake: one, at once", &woke[0].text);
    let recorded = samples::opened(&conn, &woke[0].turn);
    assert_eq!(recorded, (Some("wake".into()), Some(words::WAKE)));

    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = give(&mut conn, "ann");
    begin(&mut conn, task);
    let told = quiet::silent_turn(&mut conn, "c", "ann", Some(task), 720_000, T).unwrap();
    samples::same("silent: on a task", &told[0].text);
    let recorded = samples::opened(&conn, &told[0].turn);
    assert_eq!(recorded, (Some("silent".into()), Some(words::SILENT)));
}
