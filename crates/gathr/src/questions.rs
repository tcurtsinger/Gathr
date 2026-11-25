//! Questions an agent asks the person (`ask_person`): open until the person answers with one of its
//! choices, their own words or both, and the answer starts the asking agent's turn. Archiving the
//! chat closes them unanswered (`archive`). An agent can ask several at once (`ask_all`): one card the person answers
//! whole (`answer_all`), and one turn with every answer.

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

/// One of several questions asked together: a word or two naming it, the question, and its choices.
#[derive(Serialize, Deserialize)]
pub struct Part {
    pub topic: String,
    pub question: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
}

/// The person's answer to one of several questions: a pick of its choices, their own words or both.
#[derive(Serialize, Deserialize, Default)]
pub struct Given {
    pub pick: Option<String>,
    pub words: Option<String>,
}

/// The most questions one card holds.
pub const MOST: usize = 4;

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

/// Asks the person `parts` together for `agent` in `chat`, under `title` when given: the card's id. Its question, as
/// a line elsewhere (Needs you, other agents' chat), is the title, or "N questions", then each topic and question.
pub fn ask_all(
    conn: &Connection,
    (chat, agent): (&str, &str),
    title: &str,
    parts: &[Part],
) -> Result<i64, Error> {
    if parts.len() < 2 || parts.len() > MOST {
        return Err(Error::Refused(format!(
            "ask 2 to {MOST} questions together, or one with `what`."
        )));
    }
    for p in parts {
        let topic = p.topic.trim();
        if topic.is_empty() || topic.chars().count() > 24 {
            return Err(Error::Refused(
                "give each question a topic of a word or two.".into(),
            ));
        }
        if p.question.trim().is_empty() {
            return Err(Error::Refused(format!("say what you ask on {topic}.")));
        }
    }
    let title = match title.trim() {
        "" => format!("{} questions", parts.len()),
        t => t.to_string(),
    };
    let lines: Vec<String> = parts
        .iter()
        .map(|p| format!("- {}: {}", p.topic.trim(), first_line(&p.question)))
        .collect();
    let question = format!("{title}\n\n{}", lines.join("\n"));
    let parts = serde_json::to_string(parts)?;
    conn.execute(
        "INSERT INTO questions (chat_id, agent, question, parts, asked_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat, agent, question, parts, now_us()],
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
    let q = open(conn, id)?;
    if !q["parts"].is_null() {
        return Err(Error::Refused("answer each of its questions.".into()));
    }
    let (pick, words) = given(&q["choices"], pick, words)?;
    conn.execute(
        "UPDATE questions SET pick = ?2, words = ?3, answered_ts = ?4 WHERE id = ?1",
        params![id, pick, words, now_us()],
    )?;
    let text = format!(
        "The person answered your question \"{}\": {}",
        str(&q["question"]),
        said(pick, words)
    );
    started(conn, &q, text)
}

/// The person answers every one of question `id`'s questions, in order: the card as answered, and the asking agent's
/// turn that opens with all the answers.
pub fn answer_all(conn: &Connection, id: i64, answers: &[Given]) -> Result<(Value, Start), Error> {
    let q = open(conn, id)?;
    let Some(parts) = q["parts"].as_array() else {
        return Err(Error::Refused("it's one question: answer it alone.".into()));
    };
    if answers.len() != parts.len() {
        return Err(Error::Refused("answer each of its questions.".into()));
    }
    let mut kept = Vec::new();
    let mut lines = Vec::new();
    let mut short = Vec::new();
    for (part, a) in parts.iter().zip(answers) {
        let topic = str(&part["topic"]);
        let (pick, words) = given(&part["choices"], a.pick.as_deref(), a.words.as_deref())
            .map_err(|e| match e {
                Error::Refused(why) => Error::Refused(format!("{topic}: {why}")),
                e => e,
            })?;
        lines.push(format!(
            "- {topic}, \"{}\": {}",
            str(&part["question"]),
            said(pick, words)
        ));
        short.push(format!(
            "{topic}: {}",
            [pick, words]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
        ));
        kept.push(json!({ "pick": pick, "words": words }));
    }
    conn.execute(
        "UPDATE questions SET answers = ?2, words = ?3, answered_ts = ?4 WHERE id = ?1",
        params![
            id,
            Value::Array(kept).to_string(),
            short.join(". "),
            now_us()
        ],
    )?;
    let text = format!(
        "The person answered your questions \"{}\":\n{}",
        first_line(str(&q["question"])),
        lines.join("\n")
    );
    started(conn, &q, text)
}

/// Question `id` while it waits for the person.
fn open(conn: &Connection, id: i64) -> Result<Value, Error> {
    let sql = format!("{SELECT} WHERE q.id = ?1");
    let found = conn.prepare_cached(&sql)?.query_row([id], row).optional()?;
    let q = found.ok_or_else(|| Error::Refused("there's no such question.".into()))?;
    if !q["answered"].is_null() {
        return Err(Error::Refused("it's answered already.".into()));
    }
    if !q["closed"].is_null() {
        return Err(Error::Refused(
            "it was closed when its chat was archived.".into(),
        ));
    }
    Ok(q)
}

/// A pick of `choices` and the person's words, trimmed: refused when the pick isn't a choice or neither is given.
fn given<'a>(
    choices: &Value,
    pick: Option<&'a str>,
    words: Option<&'a str>,
) -> Result<(Option<&'a str>, Option<&'a str>), Error> {
    let pick = pick.map(str::trim).filter(|p| !p.is_empty());
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    let labels = choices.as_array().into_iter().flatten();
    let mut labels = labels.filter_map(|c| c["label"].as_str());
    if let Some(pick) = pick.filter(|p| !labels.any(|l| l == *p)) {
        return Err(Error::Refused(format!("it has no choice {pick}.")));
    }
    if pick.is_none() && words.is_none() {
        return Err(Error::Refused("pick a choice or write an answer.".into()));
    }
    Ok((pick, words))
}

/// An answer as the asking agent hears it.
fn said(pick: Option<&str>, words: Option<&str>) -> String {
    let said: Vec<String> = [
        pick.map(|p| format!("Picked: {p}")),
        words.map(|w| format!("Their words: {w}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    said.join(". ")
}

/// The asking agent's turn that opens with `text`, and question `q` as it now is.
fn started(conn: &Connection, q: &Value, text: String) -> Result<(Value, Start), Error> {
    let (chat, agent) = (str(&q["chat"]), str(&q["agent"]));
    // The person's own act, as a pick of several agents is: a pause never holds the turn it opens.
    let start = Start::queue(conn, (chat, agent), None, Opened::Picked, text)?;
    let sql = format!("{SELECT} WHERE q.id = ?1");
    let answered = conn
        .prepare_cached(&sql)?
        .query_row([q["id"].as_i64().unwrap_or_default()], row)?;
    Ok((answered, start))
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

const SELECT: &str = "SELECT q.id, q.chat_id, q.agent, COALESCE(a.name, 'An agent'), q.question,
    q.choices, q.pick, q.words, q.asked_ts, q.answered_ts, q.closed_ts, q.parts, q.answers FROM questions q LEFT JOIN agents a ON a.id = q.agent";

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
        "closed": r.get::<_, Option<i64>>(10)?.map(utc),
        "parts": json_of(r.get(11)?),
        "answers": json_of(r.get(12)?),
    }))
}

fn json_of(text: Option<String>) -> Value {
    text.and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

fn str(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}
