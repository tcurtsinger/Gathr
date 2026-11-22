//! A chat's title past its first line: the background model's name for it, and the person's rename,
//! which is final. And which model the background runs on, as the person set it.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;
use crate::store::title_from;

/// The person's pick in `settings`, `{"account", "model"}`; none means Gathr picks.
const MODEL: &str = "background_model";
/// A first message shorter than this keeps the title its first line gives it.
pub const SHORTEST: usize = 40;
/// How much of the message the model reads.
const MOST: usize = 2_000;
/// The longest title, the person's or the model's.
pub const LONGEST: usize = 60;

/// What the model is told, before the message, which it gets on its input.
pub const ASK: &str = "Name a chat from its first message, given below. Answer with the name only: \
2 to 5 words, in the message's language, no quotes, no full stop.";

/// The person's pick: the account's id and the model's, as its account lists it.
pub fn model(conn: &Connection) -> Result<Option<(String, String)>, Error> {
    let sql = "SELECT value FROM settings WHERE key = ?1";
    let saved: Option<String> = conn.query_row(sql, [MODEL], |r| r.get(0)).optional()?;
    let picked = saved.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    Ok(picked.and_then(|p| {
        let (account, model) = (p["account"].as_str()?, p["model"].as_str()?);
        Some((account.to_string(), model.to_string()))
    }))
}

/// Saves the person's pick, or `None` for Gathr's own.
pub fn set_model(conn: &Connection, picked: Option<(&str, &str)>) -> Result<(), Error> {
    match picked {
        Some((account, model)) => {
            if crate::accounts::get(conn, account)?.is_none() {
                return Err(Error::Refused("no such account".into()));
            }
            let value = serde_json::json!({ "account": account, "model": model }).to_string();
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![MODEL, value],
            )?
        }
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [MODEL])?,
    };
    Ok(())
}

/// The person's words in a message, without the lines naming its files.
pub fn words(body: &str) -> String {
    let lines = body
        .lines()
        .filter(|l| !l.trim_start().starts_with(crate::files::ATTACHED));
    lines.collect::<Vec<_>>().join("\n").trim().to_string()
}

/// What the background model reads of `message`, and the chat's title as its first line gave it,
/// when it's the chat's first message from the person and the chat still has that title.
pub fn first(
    conn: &Connection,
    chat: &str,
    message: &str,
) -> Result<Option<(String, String)>, Error> {
    let first: Option<(String, String)> = conn
        .query_row(
            "SELECT id, body FROM messages WHERE chat_id = ?1 AND role = 'user' ORDER BY ts, id LIMIT 1",
            [chat],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((_, body)) = first.filter(|(id, _)| id == message) else {
        return Ok(None);
    };
    let title: String = conn.query_row("SELECT title FROM chats WHERE id = ?1", [chat], |r| {
        r.get(0)
    })?;
    if title_from(&body).as_deref() != Some(title.as_str()) {
        return Ok(None);
    }
    let read: String = words(&body).chars().take(MOST).collect();
    Ok(Some((read, title)))
}

/// The name in the model's answer: its first line with text, without quotes around it or a full
/// stop after, when that's 1 to 60 characters.
pub fn answer(said: &str) -> Option<String> {
    let line = crate::text::first_line(said)?;
    let quotes = ['"', '\'', '`', '“', '”', '‘', '’', '«', '»'];
    let name = line.trim_matches(|c: char| quotes.contains(&c) || c.is_whitespace());
    let name = name.strip_suffix('.').unwrap_or(name).trim();
    let count = name.chars().count();
    (1..=LONGEST).contains(&count).then(|| name.to_string())
}

/// Names the chat `name`, only while its title is still `was`, what its first line gave it: a
/// rename by the person since stays. Whether it was named.
pub fn name(conn: &Connection, chat: &str, was: &str, name: &str) -> Result<bool, Error> {
    let sql = "UPDATE chats SET title = ?3 WHERE id = ?1 AND title = ?2";
    Ok(conn.execute(sql, params![chat, was, name])? == 1)
}

/// The person's rename: one line of 1 to 60 characters, trimmed.
pub fn rename(conn: &Connection, chat: &str, title: &str) -> Result<String, Error> {
    let title = title.trim();
    let count = title.chars().count();
    if count == 0 || count > LONGEST || title.contains(['\n', '\r']) {
        return Err(Error::Refused(format!(
            "A chat's name is one line of 1 to {LONGEST} characters"
        )));
    }
    let done = conn.execute(
        "UPDATE chats SET title = ?2 WHERE id = ?1",
        params![chat, title],
    )?;
    match done {
        0 => Err(Error::Refused("no such chat".into())),
        _ => Ok(title.to_string()),
    }
}
