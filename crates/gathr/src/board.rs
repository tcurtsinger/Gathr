//! The board: a chat's pieces of work, each with an owner (an agent of the chat) and one of four states.
//! Agents set its lines; Gathr checks nothing on it and says nothing about it. It starts one thing from it:
//! a member whose line still says doing when its chat goes quiet gets its turn back once (`nudge`).

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;

/// One change to the board, as `set_board_line` asks it: a new line when `line` is none.
pub struct Change {
    pub line: Option<i64>,
    pub text: Option<String>,
    /// An agent's name in the chat; the caller when a new line names none.
    pub owner: Option<String>,
    /// `to_do`, `doing`, `in_review` or `done`; a new line starts at `to_do`.
    pub state: Option<&'static str>,
    pub remove: bool,
}

/// Makes `change` to `chat`'s board for agent `by`, on the caller's transaction, and answers the board.
pub fn set(conn: &Connection, chat: &str, by: &str, change: Change) -> Result<Value, Error> {
    let text = change.text.map(|t| t.trim().to_string());
    if text.as_deref() == Some("") {
        return Err(Error::Refused("a line's text can't be empty.".into()));
    }
    let owner = match &change.owner {
        Some(name) => Some(member(conn, chat, name)?),
        None => None,
    };
    let Some(line) = change.line else {
        if change.remove {
            return Err(Error::Refused("say which line to take off.".into()));
        }
        let text = text.ok_or_else(|| Error::Refused("a new line needs its text.".into()))?;
        conn.execute(
            "INSERT INTO board_lines (chat_id, owner, text, state) VALUES (?1, ?2, ?3, ?4)",
            params![
                chat,
                owner.as_deref().unwrap_or(by),
                text,
                change.state.unwrap_or("to_do")
            ],
        )?;
        return lines(conn, chat);
    };
    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM board_lines WHERE id = ?1 AND chat_id = ?2",
            params![line, chat],
            |r| r.get(0),
        )
        .optional()?;
    if found.is_none() {
        return Err(Error::Refused(format!(
            "no line {line} on this chat's board."
        )));
    }
    if change.remove {
        conn.execute("DELETE FROM board_lines WHERE id = ?1", [line])?;
    } else {
        conn.execute(
            "UPDATE board_lines SET text = COALESCE(?2, text), owner = COALESCE(?3, owner),
             state = COALESCE(?4, state) WHERE id = ?1",
            params![line, text, owner, change.state],
        )?;
    }
    lines(conn, chat)
}

/// `chat`'s board, in the order its lines were added, each with its owner's name.
pub fn lines(conn: &Connection, chat: &str) -> Result<Value, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT b.id, b.owner, COALESCE(a.name, 'An agent'), b.text, b.state FROM board_lines b
         LEFT JOIN agents a ON a.id = b.owner WHERE b.chat_id = ?1 ORDER BY b.id",
    )?;
    let rows = statement.query_map([chat], |r| {
        let (id, owner, name): (i64, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
        let (text, state): (String, String) = (r.get(3)?, r.get(4)?);
        Ok(json!({ "id": id, "owner": owner, "name": name, "text": text, "state": state }))
    })?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<_>>()?))
}

/// The board as the tool answers it: one line each, "3 · doing · Ann · Column picker".
pub fn said(lines: &Value) -> String {
    let all = lines.as_array().map_or(&[][..], Vec::as_slice);
    if all.is_empty() {
        return "The board is empty.".into();
    }
    let line = |l: &Value| {
        format!(
            "{} · {} · {} · {}",
            l["id"],
            str(&l["state"]),
            str(&l["name"]),
            str(&l["text"])
        )
    };
    all.iter().map(line).collect::<Vec<_>>().join("\n")
}

fn str(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

/// The id of the agent of `chat` named `name`, any case.
fn member(conn: &Connection, chat: &str, name: &str) -> Result<String, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT a.id, a.name FROM chat_members m JOIN agents a ON a.id = m.agent_id
         WHERE m.chat_id = ?1 ORDER BY a.name",
    )?;
    let rows = statement.query_map([chat], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    });
    let members = rows?.collect::<rusqlite::Result<Vec<_>>>()?;
    let wanted = name.trim().trim_start_matches('@').to_lowercase();
    if let Some((id, _)) = members.iter().find(|(_, n)| n.to_lowercase() == wanted) {
        return Ok(id.clone());
    }
    let names: Vec<&str> = members.iter().map(|(_, n)| n.as_str()).collect();
    Err(Error::Refused(format!(
        "{name} isn't in this chat: the owner is one of {}.",
        names.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tool's write is what the board's route and the window read back.
    #[test]
    fn a_line_set_is_stored_and_read_back() {
        let dir = std::env::temp_dir().join(format!("gathr-board-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 0);
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('a1', 'claude', 'A', 'a', 0);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('ann', 'Ann', 'ann', 'a1', 0), ('bo', 'Bo', 'bo', 'a1', 0);
             INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES
                 ('c', 'ann', 'supervisor', 0), ('c', 'bo', 'member', 0);",
        )
        .unwrap();
        let change = |line, text: Option<&str>, owner: Option<&str>, state, remove| Change {
            line,
            text: text.map(str::to_string),
            owner: owner.map(str::to_string),
            state,
            remove,
        };
        set(
            &conn,
            "c",
            "ann",
            change(None, Some("Export"), None, None, false),
        )
        .unwrap();
        set(
            &conn,
            "c",
            "ann",
            change(None, Some("Help line"), Some("bo"), Some("doing"), false),
        )
        .unwrap();
        set(
            &conn,
            "c",
            "bo",
            change(Some(1), None, None, Some("in_review"), false),
        )
        .unwrap();
        let board = lines(&conn, "c").unwrap();
        assert_eq!(
            board,
            json!([
                { "id": 1, "owner": "ann", "name": "Ann", "text": "Export", "state": "in_review" },
                { "id": 2, "owner": "bo", "name": "Bo", "text": "Help line", "state": "doing" },
            ])
        );
        set(&conn, "c", "bo", change(Some(2), None, None, None, true)).unwrap();
        assert_eq!(lines(&conn, "c").unwrap().as_array().unwrap().len(), 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
}
