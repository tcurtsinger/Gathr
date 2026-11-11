//! Search over chat titles and message text (plans/search.md): word prefixes through the FTS5
//! index of migration 0015, newest first; titles by substring.

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::Error;

/// The longest query read, in characters; the rest is ignored.
pub const MAX_CHARS: usize = 200;
/// The most message hits one search answers.
pub const MAX_LIMIT: usize = 100;
const CHAT_HITS: usize = 8;

/// Where a match starts and ends in a snippet, private-use characters the text never shows.
const OPEN: char = '\u{E000}';
const CLOSE: char = '\u{E001}';

/// What a search found, and its sizes for the event.
pub struct Found {
    pub answer: Value,
    pub chat_hits: usize,
    pub message_hits: usize,
}

/// The query as it's searched: trimmed and cut to [`MAX_CHARS`].
pub fn clean(q: &str) -> String {
    q.trim()
        .chars()
        .take(MAX_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The FTS5 query for what was typed: each word quoted, so nothing typed is read as syntax, and
/// matched as a word prefix; every word must match. A word with no letter or digit is an empty
/// phrase, which FTS5 leaves out.
pub fn words(q: &str) -> Vec<String> {
    q.split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect()
}

/// Takes the markers out of a snippet: its text, and each match's `[start, end]` in UTF-16 units,
/// as JavaScript indexes strings.
pub fn marks(snippet: &str) -> (String, Vec<[usize; 2]>) {
    let (mut text, mut marks) = (String::with_capacity(snippet.len()), Vec::new());
    let (mut at, mut start) = (0, None);
    for c in snippet.chars() {
        match c {
            OPEN => start = Some(at),
            CLOSE => marks.extend(start.take().map(|s| [s, at])),
            _ => {
                text.push(c);
                at += c.len_utf16();
            }
        }
    }
    (text, marks)
}

/// Chats whose title holds `q`, then text messages with every word, newest first; archived chats
/// only when `archived`, each marked.
pub fn search(conn: &Connection, q: &str, archived: bool, limit: usize) -> Result<Found, Error> {
    let q = clean(q);
    if q.is_empty() {
        let answer = json!({ "chats": [], "messages": [] });
        return Ok(Found {
            answer,
            chat_hits: 0,
            message_hits: 0,
        });
    }
    let chats = chats(conn, &q, archived)?;
    let messages = messages(
        conn,
        &words(&q).join(" "),
        archived,
        limit.clamp(1, MAX_LIMIT),
    )?;
    let (chat_hits, message_hits) = (chats.len(), messages.len());
    let answer = json!({ "chats": chats, "messages": messages });
    Ok(Found {
        answer,
        chat_hits,
        message_hits,
    })
}

fn chats(conn: &Connection, q: &str, archived: bool) -> Result<Vec<Value>, Error> {
    let like = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt = conn.prepare_cached(
        "SELECT c.id, c.title, c.project_id, c.archived_ts,
                coalesce((SELECT max(m.ts) FROM messages m WHERE m.chat_id = c.id), c.created_ts) AS active
         FROM chats c
         WHERE c.title LIKE ?1 ESCAPE '\\' AND (?2 OR c.archived_ts IS NULL)
         ORDER BY active DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![like, archived, CHAT_HITS as i64], |r| {
        Ok(json!({
            "chat": r.get::<_, String>(0)?,
            "title": r.get::<_, String>(1)?,
            "project": r.get::<_, Option<String>>(2)?,
            "archived": r.get::<_, Option<i64>>(3)?.map(crate::ids::utc),
            "time": crate::ids::utc(r.get(4)?),
        }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn messages(
    conn: &Connection,
    fts: &str,
    archived: bool,
    limit: usize,
) -> Result<Vec<Value>, Error> {
    // Newest first by rowid, the order messages were saved: sorting by time instead sorts every
    // match, 15 to 20 times slower at 115k messages (plans/search.md).
    let mut stmt = conn.prepare_cached(
        "SELECT m.id, m.chat_id, c.title, c.archived_ts, m.role, t.agent_id, m.ts,
                snippet(message_search, 0, char(57344), char(57345), '…', 16)
         FROM message_search s
         JOIN messages m ON m.rowid = s.rowid
         JOIN chats c ON c.id = m.chat_id
         LEFT JOIN turns t ON t.id = m.turn_id
         WHERE message_search MATCH ?1 AND (?2 OR c.archived_ts IS NULL)
         ORDER BY s.rowid DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![fts, archived, limit as i64], |r| {
        let (snippet, marks) = marks(&r.get::<_, String>(7)?);
        Ok(json!({
            "chat": r.get::<_, String>(1)?,
            "chat_title": r.get::<_, String>(2)?,
            "archived": r.get::<_, Option<i64>>(3)?.map(crate::ids::utc),
            "message": r.get::<_, String>(0)?,
            "role": r.get::<_, String>(4)?,
            "agent": r.get::<_, Option<String>>(5)?,
            "time": crate::ids::utc(r.get(6)?),
            "snippet": snippet,
            "marks": marks,
        }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_count_utf16_units_past_emoji_and_cjk() {
        let (text, found) = marks("😀 漢 \u{E000}gravel\u{E001} and \u{E000}river\u{E001}…");
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let slice = |[s, e]: [usize; 2]| String::from_utf16(&utf16[s..e]).unwrap();
        assert_eq!(found.len(), 2, "two marks in {text:?}");
        assert_eq!(slice(found[0]), "gravel");
        assert_eq!(slice(found[1]), "river");
        assert!(!text.contains(['\u{E000}', '\u{E001}']));
    }
}

// A probe comment for the docs-only CI check.
