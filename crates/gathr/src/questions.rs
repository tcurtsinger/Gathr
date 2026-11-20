//! Questions an agent asks the person (`ask_person`): open until the person answers with one of its
//! choices, their own words or both, and the answer starts the asking agent's turn.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, utc};
use crate::store::Start;
use crate::words::Opened;

#[derive(Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub detail: Option<String>,
}

/// Asks the person `question` for `agent` in `chat`: the question's id.
pub fn ask(
    conn: &Connection,
    (chat, agent): (&str, &str),
    question: &str,
    choices: &[Choice],
) -> Result<i64, Error> {
    let question = question.trim();
    if question.is_empty() {
        return Err(Error::Refused("say what you ask.".into()));
    }
    let choices = match choices.is_empty() {
        true => None,
        false => Some(serde_json::to_string(choices)?),
    };
    conn.execute(
        "INSERT INTO questions (chat_id, agent, question, choices, asked_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat, agent, question, choices, now_us()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// `chat`'s questions in the order they were asked, open and answered, each with its agent's name.
pub fn of_chat(conn: &Connection, chat: &str) -> Result<Value, Error> {
    let sql = format!("{SELECT} WHERE q.chat_id = ?1 ORDER BY q.id");
    let mut statement = conn.prepare_cached(&sql)?;
    let rows = statement.query_map([chat], row)?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<_>>()?))
}

/// The person answers question `id` with `pick`, one of its choices, `words` of their own, or both:
/// the question as answered, and the asking agent's turn that opens with the answer.
pub fn answer(
    conn: &Connection,
    id: i64,
    pick: Option<&str>,
    words: Option<&str>,
) -> Result<(Value, Start), Error> {
    let sql = format!("{SELECT} WHERE q.id = ?1");
    let found = conn.prepare_cached(&sql)?.query_row([id], row).optional()?;
    let q = found.ok_or_else(|| Error::Refused("there's no such question.".into()))?;
    if !q["answered"].is_null() {
        return Err(Error::Refused("it's answered already.".into()));
    }
    let pick = pick.map(str::trim).filter(|p| !p.is_empty());
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    let labels = q["choices"].as_array().into_iter().flatten();
    let mut labels = labels.filter_map(|c| c["label"].as_str());
    if let Some(pick) = pick.filter(|p| !labels.any(|l| l == *p)) {
        return Err(Error::Refused(format!("it has no choice {pick}.")));
    }
    let said: Vec<String> = [
        pick.map(|p| format!("Picked: {p}")),
        words.map(|w| format!("Their words: {w}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if said.is_empty() {
        return Err(Error::Refused("pick a choice or write an answer.".into()));
    }
    conn.execute(
        "UPDATE questions SET pick = ?2, words = ?3, answered_ts = ?4 WHERE id = ?1",
        params![id, pick, words, now_us()],
    )?;
    let (chat, agent) = (str(&q["chat"]), str(&q["agent"]));
    let text = format!(
        "The person answered your question \"{}\": {}",
        str(&q["question"]),
        said.join(". ")
    );
    // The person's own act, as a pick of several agents is: a pause never holds the turn it opens.
    let start = Start::queue(conn, (chat, agent), None, Opened::Picked, text)?;
    let sql = format!("{SELECT} WHERE q.id = ?1");
    let answered = conn.prepare_cached(&sql)?.query_row([id], row)?;
    Ok((answered, start))
}

const SELECT: &str = "SELECT q.id, q.chat_id, q.agent, COALESCE(a.name, 'An agent'), q.question,
    q.choices, q.pick, q.words, q.asked_ts, q.answered_ts FROM questions q LEFT JOIN agents a ON a.id = q.agent";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let choices: Option<String> = r.get(5)?;
    let choices = choices.and_then(|c| serde_json::from_str::<Value>(&c).ok());
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "chat": r.get::<_, String>(1)?,
        "agent": r.get::<_, String>(2)?,
        "name": r.get::<_, String>(3)?,
        "question": r.get::<_, String>(4)?,
        "choices": choices.unwrap_or_else(|| json!([])),
        "pick": r.get::<_, Option<String>>(6)?,
        "words": r.get::<_, Option<String>>(7)?,
        "asked": utc(r.get(8)?),
        "answered": r.get::<_, Option<i64>>(9)?.map(utc),
    }))
}

fn str(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}
