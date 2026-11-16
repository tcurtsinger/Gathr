//! A task's brief (0059): why, what done means, how it's proved, where the work goes and how big it
//! is. A work task an agent gives needs the first four, and each field has its limit.

use super::{Kind, TITLE_MOST, Task, refused};
use crate::Error;
use crate::words::Opened;

pub const DONE_MOST: usize = 300;
pub const PROOF_MOST: usize = 300;
pub const WHERE_MOST: usize = 200;
pub const SIZE_MOST: usize = 200;

/// A new task.
pub struct Give<'a> {
    pub chat: &'a str,
    pub giver: Option<&'a str>,
    pub owner: &'a str,
    pub given_in: Option<&'a str>,
    pub parent: Option<i64>,
    pub kind: Kind,
    pub what: &'a str,
    pub brief: Brief<'a>,
    pub after: &'a [i64],
    pub pr: Option<i64>,
    /// Given in passing: the giver's own task doesn't wait on it, and it's heard back on by itself.
    pub in_passing: bool,
}

/// What a task's giver says of it beyond its title.
#[derive(Clone, Copy, Default)]
pub struct Brief<'a> {
    pub why: Option<&'a str>,
    pub done: Option<&'a str>,
    pub proof: Option<&'a str>,
    /// Where the work goes: the folder and branch, or "no code".
    pub place: Option<&'a str>,
    /// The files or area it touches, and the rough lines it adds.
    pub size: Option<(&'a str, i64)>,
}

/// A field as saved: trimmed, and none when blank.
pub(super) fn given(field: Option<&str>) -> Option<&str> {
    field.map(str::trim).filter(|f| !f.is_empty())
}

/// `size` and `lines` as one: both or neither.
pub fn sized(size: Option<&str>, lines: Option<i64>) -> Result<Option<(&str, i64)>, Error> {
    match (given(size), lines) {
        (Some(size), Some(lines)) => Ok(Some((size, lines))),
        (None, None) => Ok(None),
        _ => Err(refused(
            "`size` and `lines` go together: `size` names the files or area, `lines` the rough lines it adds.",
        )),
    }
}

/// A task's title and brief as they would be saved: its title, then a review's fields, then what a
/// work task lacks, then each field's length, then its lines.
pub(crate) fn checked(kind: Kind, what: &str, b: &Brief) -> Result<(), Error> {
    let what = what.trim();
    if what.is_empty() || what.contains('\n') {
        return Err(refused(format!(
            "`what` is one line of 1 to {TITLE_MOST} characters"
        )));
    }
    fits("what", what, TITLE_MOST)?;
    let (done, proof, place) = (given(b.done), given(b.proof), given(b.place));
    let size = b.size.map(|(size, _)| size.trim());
    if kind == Kind::Review && (done.or(proof).or(place).is_some() || size.is_some()) {
        return Err(refused(
            "A review takes none of done, proof, where or size: Gathr says what its done and proof are.",
        ));
    }
    let fields = [
        ("`why`", given(b.why)),
        ("`done`", done),
        ("`proof`", proof),
        ("`where`", place),
    ];
    let missing: Vec<&str> = fields
        .iter()
        .filter(|f| f.1.is_none())
        .map(|f| f.0)
        .collect();
    if kind == Kind::Work && !missing.is_empty() {
        return Err(refused(format!(
            "A work task needs why, done, proof and where. Missing: {}. `done` says what is true when it's \
             done, `proof` how that is shown, `where` the folder and branch the work goes in, or \"no code\".",
            crate::waiting::words::list(&missing)
        )));
    }
    let limits = [
        ("done", done, DONE_MOST),
        ("proof", proof, PROOF_MOST),
        ("where", place, WHERE_MOST),
        ("size", size, SIZE_MOST),
    ];
    for (name, field, most) in limits {
        field.map_or(Ok(()), |field| fits(name, field, most))?;
    }
    if b.size.is_some_and(|(_, lines)| lines < 1) {
        return Err(refused(
            "`lines` is the rough number of lines it adds: 1 or more.",
        ));
    }
    Ok(())
}

fn fits(name: &str, field: &str, most: usize) -> Result<(), Error> {
    let long = field.chars().count();
    match long > most {
        true => Err(refused(format!(
            "`{name}` is {long} characters: cut {} to fit {most}, and put the details in `why`",
            long - most
        ))),
        false => Ok(()),
    }
}

/// A work task's brief says each field it has, after its why. A review's and a request's say none:
/// their forms are their own.
pub(super) fn lines(t: &Task) -> Vec<String> {
    if t.kind != Kind::Work {
        return Vec::new();
    }
    let fields = [
        ("Done when", &t.done),
        ("Proof", &t.proof),
        ("Where", &t.place),
    ];
    let mut lines: Vec<String> = fields
        .iter()
        .filter_map(|(label, field)| Some(format!("{label}: {}", field.as_ref()?)))
        .collect();
    if let (Some(size), Some(n)) = (&t.size, t.size_lines) {
        lines.push(format!("Size: {size}, about {n} lines"));
    }
    lines
}

/// The words a task's brief is, by its kind; a plan check's are its own (0062).
pub(super) fn opened(t: &Task) -> Opened {
    if t.checks.is_some() {
        return Opened::PlanCheck;
    }
    match t.kind {
        Kind::Review => Opened::ReviewBrief,
        Kind::PersonRequest => Opened::RequestBrief,
        _ => Opened::Brief,
    }
}
