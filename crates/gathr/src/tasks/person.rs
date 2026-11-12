//! The person's tasks (F9, docs/decisions/0010-coordination-as-state.md). Only a chat's Supervisor
//! gives them: a member's ask reaches the Supervisor first, as a `person_request` it checks, then
//! files, folds into one the person already has, or answers itself. The person's answers go back to
//! the Supervisor as they're sent, and from there to the member who needed them.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Kind, Start, State, Task, WHAT_MOST, cascade, read, ready, refused, set, words};
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::team::Role;

/// The most choices a question offers, and the longest label: picked at a glance.
const CHOICES_MOST: usize = 8;
const LABEL_MOST: usize = 80;

/// How the person answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    /// Something to do: Done.
    Done,
    Yesno,
    /// One of its choices.
    Choice,
    /// Any number of its choices.
    Choices,
    /// Words.
    Text,
}

const FORMS: [(Form, &str); 5] = [
    (Form::Done, "done"),
    (Form::Yesno, "yesno"),
    (Form::Choice, "choice"),
    (Form::Choices, "choices"),
    (Form::Text, "text"),
];

impl Form {
    pub fn named(text: &str) -> Option<Form> {
        FORMS.iter().find(|(_, n)| *n == text).map(|(f, _)| *f)
    }

    pub fn as_str(self) -> &'static str {
        FORMS
            .iter()
            .find(|(f, _)| *f == self)
            .map_or("", |(_, n)| n)
    }

    /// What answers it, for a refusal.
    fn needs(self) -> &'static str {
        match self {
            Form::Done => "done",
            Form::Yesno => "yes or no",
            Form::Choice => "one of its choices",
            Form::Choices => "at least one of its choices",
            Form::Text => "words",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A file given with a task: a copy made when it was given, so the person reads what the agent
/// meant even after its worktree moves on. The window gets its name and size; the path stays here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub name: String,
    pub size: u64,
    #[serde(default, skip_serializing)]
    pub path: String,
}

/// An agent's ask for the person.
pub struct Ask<'a> {
    pub chat: &'a str,
    pub giver: &'a str,
    pub given_in: Option<&'a str>,
    pub parent: Option<i64>,
    /// None only with `request`: the request is filed as it is.
    pub what: Option<&'a str>,
    pub why: Option<&'a str>,
    /// None: `choice` with choices, else `text` for a question, else `done`.
    pub form: Option<Form>,
    /// The giver's pick first.
    pub choices: Vec<Choice>,
    pub files: Vec<File>,
    /// The member's request this is filed for.
    pub request: Option<i64>,
}

/// The person's answer to one task. Words alone answer any task.
#[derive(Debug, Default, Deserialize)]
pub struct Answer {
    pub task: i64,
    #[serde(default)]
    pub pick: Vec<String>,
    pub yes: Option<bool>,
    #[serde(default)]
    pub done: bool,
    pub text: Option<String>,
}

/// Files the ask for the person when the giver is the chat's lead (its Supervisor, or the chat's only
/// agent); from anyone else it's a `person_request` to the Supervisor, with its turn.
pub fn ask(conn: &mut Connection, a: Ask, trace: &str) -> Result<(Task, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    crate::archive::refuse_archived(&tx, a.chat)?;
    let lead = lead_of(&tx, a.chat, a.giver)?;
    let from = match a.request {
        Some(r) => Some(request_for(&tx, a.chat, a.giver, r)?),
        None => None,
    };
    let what = match (a.what.map(str::trim).filter(|w| !w.is_empty()), &from) {
        (Some(what), _) => what.to_string(),
        (None, Some(r)) => r.what.clone(),
        (None, None) => return Err(refused("say `what` the person is asked")),
    };
    if what.contains('\n') || what.chars().count() > WHAT_MOST {
        return Err(refused(format!(
            "`what` is one line of 1 to {WHAT_MOST} characters"
        )));
    }
    // Without `what`, the request goes to the person as the member wrote it.
    let (why, choices, form) = match (&from, a.what.is_none()) {
        (Some(r), true) => (r.why.clone(), r.choices.clone(), r.form),
        _ => (a.why.map(String::from), a.choices, a.form),
    };
    let form = form.unwrap_or(match (choices.is_empty(), what.ends_with('?')) {
        (false, _) => Form::Choice,
        (true, true) => Form::Text,
        (true, false) => Form::Done,
    });
    check_choices(form, &choices)?;
    let mut files = a.files;
    if files.is_empty()
        && let Some(r) = &from
    {
        files = r.files.clone();
    }
    let to_person = lead == a.giver;
    let (kind, owner) = match to_person {
        true => (Kind::Person, None),
        false => (Kind::PersonRequest, Some(lead.as_str())),
    };
    let now = now_us();
    tx.execute(
        "INSERT INTO tasks (chat_id, giver, owner, given_in, parent, kind, what, why, state, form,
                            choices, files, request, created_ts, updated_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'waiting', ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            a.chat,
            a.giver,
            owner,
            a.given_in,
            a.parent,
            kind.as_str(),
            what,
            why.as_deref().map(str::trim).filter(|w| !w.is_empty()),
            form.as_str(),
            (!choices.is_empty()).then(|| json!(choices).to_string()),
            (!files.is_empty()).then(|| stored(&files)),
            from.as_ref().map(|r| r.number),
            now
        ],
    )?;
    let number = tx.last_insert_rowid();
    let starts = match to_person {
        // The person's is open from the start: there's no turn to queue.
        true => {
            set(&tx, number, State::Ready, None, trace)?;
            Vec::new()
        }
        false => vec![ready(&tx, number, None)?],
    };
    let task = read(&tx, number)?;
    Event::point("gathr.task.give")
        .trace(trace)
        .chat(a.chat)
        .task(number)
        .ok()
        .attr("gathr.task.giver", a.giver)
        .attr("gathr.task.owner", owner.unwrap_or("person"))
        .attr("gathr.task.kind", kind.as_str())
        .attr("gathr.task.request", task.request)
        .insert(&tx)?;
    tx.commit()?;
    Ok((task, starts))
}

/// What a Send did: the tasks it answered, those it couldn't with why (they stay open), and the
/// turns it starts.
#[derive(Debug, Default)]
pub struct Sent {
    pub answered: Vec<Task>,
    pub refused: Vec<(i64, String)>,
    pub starts: Vec<Start>,
}

/// The person answers, several at once as the pager's Send does: each is answered or refused on
/// its own, so one that's wrong stays open and the rest go. Each giver hears back once on what
/// this Send answered.
pub fn answer(conn: &mut Connection, answers: &[Answer], trace: &str) -> Result<Sent, Error> {
    answer_as(conn, answers, None, trace)
}

/// `answer`, by the person (`by` none) or by an agent for the person (0043 C5a): never one the
/// agent asked itself, or asked the Supervisor to file.
pub fn answer_as(
    conn: &mut Connection,
    answers: &[Answer],
    by: Option<&str>,
    trace: &str,
) -> Result<Sent, Error> {
    let tx = conn.transaction()?;
    let mut sent = Sent::default();
    let mut numbers: Vec<i64> = Vec::new();
    for a in answers {
        if numbers.contains(&a.task) {
            sent.refused
                .push((a.task, format!("#{} is answered twice", a.task)));
            continue;
        }
        match answer_one(&tx, a, by, trace) {
            Ok(()) => numbers.push(a.task),
            Err(Error::Refused(why)) => {
                Event::point("gathr.task.answer")
                    .trace(trace)
                    .task(a.task)
                    .error("refused")
                    .attr("why", why.as_str())
                    .attr("by", by)
                    .insert(&tx)?;
                sent.refused.push((a.task, why));
            }
            Err(e) => return Err(e),
        }
    }
    sent.starts = heard(&tx, &numbers, None, trace)?;
    for &n in &numbers {
        sent.starts.extend(cascade(&tx, n, trace)?);
    }
    sent.answered = numbers
        .iter()
        .map(|&n| read(&tx, n))
        .collect::<Result<_, _>>()?;
    tx.commit()?;
    Ok(sent)
}

fn answer_one(tx: &Transaction, a: &Answer, by: Option<&str>, trace: &str) -> Result<(), Error> {
    let t = read(tx, a.task).map_err(|_| refused(format!("there's no task #{}", a.task)))?;
    if t.kind != Kind::Person {
        return Err(refused(format!("#{} isn't yours to answer", a.task)));
    }
    if t.state != State::Ready {
        let state = t.state.as_str();
        return Err(refused(format!("#{} is already {state}", a.task)));
    }
    let mut line = answer_line(&t, a)?;
    if let Some(agent) = by {
        let member = match t.request {
            Some(r) => read(tx, r)?.giver,
            None => None,
        };
        if [&t.giver, &member]
            .iter()
            .any(|g| g.as_deref() == Some(agent))
        {
            return Err(refused(format!(
                "#{} is its own question to the person: an agent never answers its own question to the person,                  the person does",
                a.task
            )));
        }
        line = format!(
            "{line} (answered by {} for the person)",
            words::name_of(tx, agent)?
        );
    }
    let sent = json!({ "pick": a.pick, "yes": a.yes, "done": a.done, "text": a.text, "by": by });
    tx.execute(
        "UPDATE tasks SET answer = ?2 WHERE number = ?1",
        params![a.task, sent.to_string()],
    )?;
    set(tx, a.task, State::Done, Some(&line), trace)
}

/// One of the person's open tasks, with its chat's title and the member it was asked for.
#[derive(Debug, Serialize)]
pub struct Open {
    #[serde(flatten)]
    pub task: Task,
    pub chat_title: String,
    /// The member whose request it was filed for; none when the Supervisor asked for itself.
    pub for_agent: Option<String>,
}

/// The person's open tasks, across chats or in one, oldest first.
pub fn open(conn: &Connection, chat: Option<&str>) -> Result<Vec<Open>, Error> {
    let found: Vec<(i64, String, Option<String>)> = {
        let mut stmt = conn.prepare_cached(
            "SELECT t.number, c.title, r.giver FROM tasks t JOIN chats c ON c.id = t.chat_id
             LEFT JOIN tasks r ON r.number = t.request
             WHERE t.kind = 'person' AND t.state = 'ready' AND (?1 IS NULL OR t.chat_id = ?1)
             ORDER BY t.number",
        )?;
        let rows = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    found
        .into_iter()
        .map(|(n, chat_title, for_agent)| {
            Ok(Open {
                task: read(conn, n)?,
                chat_title,
                for_agent,
            })
        })
        .collect()
}

/// The person's tasks settled (answered or cancelled): each giver hears back once on those it gave
/// in one turn, on its own task from that turn while it's open, and is told to pass an answer back
/// to the member who asked. A giver that cancelled its own isn't told.
pub(super) fn heard(
    tx: &Transaction,
    numbers: &[i64],
    by: Option<&str>,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let mut groups: Vec<(String, Option<String>, Vec<Task>)> = Vec::new();
    for &n in numbers {
        tx.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [n])?;
        let t = read(tx, n)?;
        let Some(giver) = t.giver.clone() else {
            continue;
        };
        if by == Some(giver.as_str()) {
            continue;
        }
        match groups
            .iter_mut()
            .find(|(g, turn, _)| *g == giver && *turn == t.given_in)
        {
            Some((_, _, list)) => list.push(t),
            None => groups.push((giver, t.given_in.clone(), vec![t])),
        }
    }
    let mut starts = Vec::new();
    for (giver, turn, settled) in groups {
        let chat = settled[0].chat.clone();
        let own: Option<i64> = match &turn {
            Some(turn) => tx
                .query_row(
                    "SELECT t.number FROM turns u JOIN tasks t ON t.number = u.task
                     WHERE u.id = ?1 AND t.state NOT IN ('done', 'cancelled')",
                    [turn],
                    |r| r.get(0),
                )
                .optional()?,
            None => None,
        };
        Event::point("gathr.task.answered")
            .trace(trace)
            .chat(&chat)
            .ok()
            .attr("gen_ai.agent.id", giver.as_str())
            .attr("gathr.task.count", settled.len())
            .insert(tx)?;
        let text = heard_words(tx, &settled)?;
        let start = Start::queue(tx, (&chat, &giver), (own, turn.as_deref()), text)?;
        for t in &settled {
            tx.execute(
                "UPDATE tasks SET heard_in = ?2 WHERE number = ?1",
                params![t.number, start.turn],
            )?;
        }
        starts.push(start);
    }
    Ok(starts)
}

/// The lines a Supervisor's turn on a member's request adds to its brief: the question, and what
/// to do with it.
pub(super) fn request_lines(t: &Task, member: &str) -> Vec<String> {
    let mut lines = vec![format!(
        "{member} needs the person's answer. Check it's the person's call, not already answered, and the pick is sound."
    )];
    lines.extend(form_lines(t));
    lines.push(format!(
        "Use ask_person with for_task {n} to file it; omit `what` to forward it unchanged. If an existing question \
            covers it, use finish_task blocked with on_task set to that question. To answer it yourself, use \
            finish_task done with the answer in note. When the person answers, finish #{n} done with their answer in \
            note to pass it back to {member}.",
        n = t.number
    ));
    lines
}

fn form_lines(t: &Task) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(form) = t.form {
        lines.push(format!("Answer: {}", form.needs()));
    }
    for (i, c) in t.choices.iter().enumerate() {
        let mine = if i == 0 { " (their pick)" } else { "" };
        let detail = c
            .detail
            .as_ref()
            .map_or(String::new(), |d| format!(": {d}"));
        lines.push(format!("{}. {}{mine}{detail}", i + 1, c.label));
    }
    if !t.files.is_empty() {
        let names: Vec<&str> = t.files.iter().map(|f| f.name.as_str()).collect();
        lines.push(format!("Files: {}", names.join(", ")));
    }
    lines
}

fn heard_words(conn: &Connection, settled: &[Task]) -> Result<String, Error> {
    let mut lines = vec!["The person answered:".to_string()];
    for t in settled {
        let how = match (t.state, &t.reason) {
            (State::Done, Some(line)) => line.clone(),
            (state, Some(why)) => format!("{} ({why})", state.as_str()),
            (state, None) => state.as_str().to_string(),
        };
        lines.push(format!("- #{} {}: {how}", t.number, t.what));
    }
    for t in settled {
        let Some(r) = t.request else { continue };
        let asker = match read(conn, r)?.giver {
            Some(member) => words::name_of(conn, &member)?,
            None => "the member".to_string(),
        };
        lines.push(format!(
            "#{} answers the request from {asker}. Use finish_task to mark #{r} done and put the answer in note.",
            t.number
        ));
    }
    lines.push("Continue.".to_string());
    Ok(lines.join("\n"))
}

/// The answer in a line, or why it doesn't answer the task.
fn answer_line(t: &Task, a: &Answer) -> Result<String, Error> {
    let form = t.form.unwrap_or(Form::Done);
    let labels: Vec<&str> = t.choices.iter().map(|c| c.label.as_str()).collect();
    if let Some(wrong) = a.pick.iter().find(|p| !labels.contains(&p.as_str())) {
        return Err(refused(format!("#{} has no choice {wrong}", t.number)));
    }
    let picked = match form {
        Form::Done => a.done.then(|| "Done".to_string()),
        Form::Yesno => a.yes.map(|y| if y { "Yes" } else { "No" }.to_string()),
        Form::Choice if a.pick.len() == 1 => Some(a.pick[0].clone()),
        Form::Choices if !a.pick.is_empty() => Some(a.pick.join(", ")),
        _ => None,
    };
    let text = a.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    match (picked, text) {
        (Some(p), Some(words)) => Ok(format!("{p}: {words}")),
        (Some(p), None) => Ok(p),
        (None, Some(words)) => Ok(words.to_string()),
        (None, None) => Err(refused(format!(
            "Answer #{} with {}, or enter text",
            t.number,
            form.needs()
        ))),
    }
}

fn check_choices(form: Form, choices: &[Choice]) -> Result<(), Error> {
    let picks = matches!(form, Form::Choice | Form::Choices);
    if picks && choices.len() < 2 {
        return Err(refused(
            "Offer at least 2 choices, with your recommendation first",
        ));
    }
    if !picks && !choices.is_empty() {
        return Err(refused(format!(
            "choices go with form choice or choices, not {}",
            form.as_str()
        )));
    }
    if choices.len() > CHOICES_MOST {
        return Err(refused(format!("at most {CHOICES_MOST} choices")));
    }
    for (i, c) in choices.iter().enumerate() {
        let label = c.label.trim();
        if label.is_empty() || label.chars().count() > LABEL_MOST || label != c.label {
            return Err(refused(format!(
                "Choice labels need 1 to {LABEL_MOST} characters, with no leading or trailing spaces"
            )));
        }
        if choices[..i].iter().any(|o| o.label == c.label) {
            return Err(refused(format!(
                "The choice {label} appears more than once"
            )));
        }
    }
    Ok(())
}

/// Who gives the person tasks in this chat: its Supervisor, or the asking agent when it's the
/// chat's only one.
fn lead_of(conn: &Connection, chat: &str, giver: &str) -> Result<String, Error> {
    let members = crate::team::members(conn, chat)?;
    if members.len() > 1 && !members.iter().any(|m| m.agent == giver) {
        return Err(refused("you aren't one of this chat's agents"));
    }
    let lead = members.iter().find(|m| m.role == Role::Supervisor);
    Ok(lead.map_or_else(|| giver.to_string(), |m| m.agent.clone()))
}

/// The member's request a person task is filed for: in this chat, the giver's, and open.
fn request_for(conn: &Connection, chat: &str, giver: &str, r: i64) -> Result<Task, Error> {
    let t = read(conn, r).map_err(|_| refused(format!("there's no task #{r}")))?;
    if t.chat != chat || t.kind != Kind::PersonRequest {
        return Err(refused(format!(
            "#{r} isn't a request for the person in this chat"
        )));
    }
    if t.owner.as_deref() != Some(giver) {
        return Err(refused(format!("#{r} isn't yours")));
    }
    if matches!(t.state, State::Done | State::Cancelled) {
        return Err(refused(format!("#{r} is already {}", t.state.as_str())));
    }
    Ok(t)
}

fn stored(files: &[File]) -> String {
    let all: Vec<_> = files
        .iter()
        .map(|f| json!({ "name": f.name, "size": f.size, "path": f.path }))
        .collect();
    json!(all).to_string()
}

#[cfg(test)]
mod tests;
