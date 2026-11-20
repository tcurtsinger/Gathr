//! Gathr's words for the open check (0080), adapted from SerenityOS's `checkText` and its settle
//! text: what a checking member reads, and what wakes the lead for its last word.

use rusqlite::{Connection, OptionalExtension};

use crate::Error;
use crate::tasks::name_of;

/// How a check that holds up or finds nothing ends, and how an objection reads. The answer is named
/// by `answer`, never by its id, which an agent then wrote into its words (0082). In a room chat an
/// objection starts with REPLY, which files it under the answer.
fn verdict(author: &str, room: bool) -> String {
    let say = match room {
        true => format!("start with REPLY on a line of its own to comment on {author}'s message"),
        false => format!("quoting the line you answer as \"> {author}: their words\""),
    };
    format!(
        "If it holds up, call react with message \"answer\" and end your turn: that thumbs-up is your whole check, \
         so write nothing, not even what you checked. If something in it is wrong, missing or claimed without \
         evidence, say what in a few lines with your evidence, {say}."
    )
}

/// What the person asked: their message, or for words they sent a plan back with, the ask's first
/// message and those words.
fn asked(conn: &Connection, asked: &str) -> Result<String, Error> {
    let (role, body, first): (String, String, String) = conn.query_row(
        "SELECT m.role, m.body, coalesce((SELECT a.body FROM messages a WHERE a.id = u.ask_id), '')
         FROM messages m JOIN turns u ON u.id = m.turn_id WHERE m.id = ?1",
        [asked],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if role == "user" {
        return Ok(body);
    }
    let words: Option<String> = conn
        .query_row(
            "SELECT words FROM plan_versions WHERE decided_line = ?1",
            [asked],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(match words {
        Some(words) => format!("{first}\nThen they sent the plan back: {words}"),
        None => first,
    })
}

/// A member's check of `answer`, the lead's answer to the person's message `asked`: a plan version
/// is checked for what a plan gets wrong, any other answer as SerenityOS checks one.
pub(super) fn check(
    conn: &Connection,
    (asked_id, answer): (&str, &str),
    lead: &str,
    room: bool,
) -> Result<String, Error> {
    let author = name_of(conn, lead)?;
    let body: String =
        conn.query_row("SELECT body FROM messages WHERE id = ?1", [answer], |r| {
            r.get(0)
        })?;
    let version: Option<(i64, String)> = conn
        .query_row(
            "SELECT n, title FROM plan_versions WHERE message_id = ?1",
            [answer],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let asked = asked(conn, asked_id)?;
    let lines = match version {
        Some((n, title)) => [
            format!(
                "Check version {n} of {author}'s plan \"{title}\" before the person sees it. Don't redo it: say what \
                 is wrong or missing (a wrong fact or file, a missed step or caller, a part given to the wrong member, \
                 a proof that proves nothing) and what the ask doesn't need, with file and line."
            ),
            format!("The person asked: {asked}"),
            format!("The plan:\n{body}"),
            verdict(&author, room),
        ],
        None => [
            format!(
                "Check {author}'s answer before the person relies on it. Don't redo the work, but where it says \
                 something was built or changed, check that part by running it and using it: reading the diff alone \
                 isn't a check, and \"it works\" with no proof is a claim without evidence."
            ),
            format!("The person asked: {asked}"),
            format!("{author}'s answer:\n{body}"),
            format!(
                "{} If there's nothing to check, such as small talk, write nothing.",
                verdict(&author, room)
            ),
        ],
    };
    Ok(lines.join("\n"))
}

/// The lead's last word on its answer `answer`, with each member's message that questioned it.
pub(super) fn last_word(
    conn: &Connection,
    answer: &str,
    objected: &[(String, String)],
) -> Result<String, Error> {
    let mut names = Vec::new();
    let mut said = Vec::new();
    for (agent, words) in objected {
        let name = name_of(conn, agent)?;
        said.push(format!("{name}: {words}"));
        names.push(name);
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let plan: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM plan_versions WHERE message_id = ?1)",
        [answer],
        |r| r.get(0),
    )?;
    let mut lines = vec![format!(
        "{} questioned your answer:",
        crate::waiting::words::list(&names)
    )];
    lines.push(said.join("\n\n"));
    lines.push(
        "This is your last word on it: look at each point, correct your answer if they're right, or say plainly \
         why it stands. A few lines, to the person."
            .to_string(),
    );
    if plan {
        lines.push(
            "To change the plan, write its next version with plan: the person sees the newest when this turn ends."
                .to_string(),
        );
    }
    Ok(lines.join("\n"))
}
