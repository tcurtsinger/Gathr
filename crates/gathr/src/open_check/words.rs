//! Gathr's words for the open check (0080), adapted from SerenityOS's `checkText` and its settle
//! text: what a checking member reads, and what wakes the lead for its last word.

use rusqlite::Connection;

use crate::Error;
use crate::agents::name_of;

/// How a check that holds up or finds nothing ends, and how an objection reads. The answer is named
/// by `answer`, never by its id, which an agent then wrote into its words (0082). An objection
/// starts with REPLY, which files it under the answer.
fn verdict(author: &str) -> String {
    format!(
        "If it holds up, call react with message \"answer\", then end your turn with only NO_REPLY: that thumbs-up \
         is your whole check, so write nothing else, not even what you checked. If something in it is wrong, \
         missing or claimed without evidence, say what in a few lines with your evidence, start with REPLY on a \
         line of its own to comment on {author}'s message. Object only to what would mislead the person: wording \
         you would have put differently is not a reason."
    )
}

/// A member's check of `answer`, the lead's answer to `asked`, the person's message or Gathr's line
/// with their answer to a question, as SerenityOS checks one.
pub(super) fn check(
    conn: &Connection,
    (asked, answer): (&str, &str),
    lead: &str,
) -> Result<String, Error> {
    let author = name_of(conn, lead)?;
    let body = |id: &str| -> Result<String, Error> {
        Ok(
            conn.query_row("SELECT body FROM messages WHERE id = ?1", [id], |r| {
                r.get(0)
            })?,
        )
    };
    // The person's role, but an AI app outside Gathr sent it: asked by the app, never by the person.
    let (by_person, app): (bool, Option<String>) = conn.query_row(
        "SELECT m.role = 'user', f.app FROM messages m LEFT JOIN from_app f ON f.message_id = m.id WHERE m.id = ?1",
        [asked],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let asked = match (by_person, app) {
        (true, Some(app)) => format!("{} asked: {}", crate::from_app::speaker(&app), body(asked)?),
        (true, None) => format!("The person asked: {}", body(asked)?),
        (false, _) => body(asked)?,
    };
    let lines = [
        format!(
            "Check {author}'s answer before the person relies on it, by reading: the answer, what was said in the \
             room, and the change itself. Don't run or rebuild the work. Run something yourself only when what \
             you read looks wrong, or a claim has nothing behind it."
        ),
        asked,
        format!("{author}'s answer:\n{}", body(answer)?),
        format!(
            "{} If there's nothing to check, such as small talk, write only NO_REPLY.",
            verdict(&author)
        ),
    ];
    Ok(lines.join("\n"))
}

/// The lead's last word on its answer, with each member's message that questioned it.
pub(super) fn last_word(conn: &Connection, objected: &[(String, String)]) -> Result<String, Error> {
    let mut names = Vec::new();
    let mut said = Vec::new();
    for (agent, words) in objected {
        let name = name_of(conn, agent)?;
        said.push(format!("{name}: {words}"));
        names.push(name);
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let lines = [
        format!("{} questioned your answer:", crate::text::list(&names)),
        said.join("\n\n"),
        "This is your last word on it: look at each point, correct your answer if they're right, or say plainly \
         why it stands. A few lines, to the person."
            .to_string(),
    ];
    Ok(lines.join("\n"))
}
