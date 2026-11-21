//! What an agent hears of its chat when its turn opens (0081, after SerenityOS's catch-up): the
//! chat's lines as `Name: words`, with the turn's own ask. A turn that starts a new conversation hears
//! the last 30; one that goes on in its conversation hears what came since the agent's last turn,
//! without the lines it said and read there. Never the message that opened the turn, a tool row,
//! an agent's working note (`SAID`) or Gathr's copy of a message. The lead hears every line whole;
//! a member hears whole the person's lines, Gathr's, the lead's and those that name it, and another
//! member's cut to its first sentence. A whole line is capped as `read_chat` caps it.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;
use crate::team::{self, Member, Role};
use crate::text;

/// The word an agent starts its words with, on a line of its own, when they answer a teammate.
pub const REPLY: &str = "REPLY";

/// The word an agent writes, alone, when it has nothing to add: nothing is posted.
pub const NO_REPLY: &str = "NO_REPLY";

/// How the start of an agent's words reads, as far as it has come.
#[derive(Debug, PartialEq)]
pub enum Start<'a> {
    /// Still possibly REPLY or NO_REPLY: no line has ended yet.
    Undecided,
    Main,
    /// REPLY on its own line: the words after it.
    Reply(&'a str),
    /// NO_REPLY on its own line with more after it: those words, a main message.
    NoReply(&'a str),
}

fn no_reply(line: &str) -> bool {
    let line = line.trim();
    line.strip_suffix('.').unwrap_or(line) == NO_REPLY
}

pub fn start(words: &str) -> Start<'_> {
    let words = words.trim_start();
    match words.split_once('\n') {
        Some((first, rest)) if first.trim() == REPLY => Start::Reply(rest),
        Some((first, rest)) if no_reply(first) => Start::NoReply(rest),
        Some(_) => Start::Main,
        None if REPLY.starts_with(words.trim_end()) => Start::Undecided,
        None if NO_REPLY.starts_with(words.trim_end()) || no_reply(words) => Start::Undecided,
        None => Start::Main,
    }
}

/// An agent's words as they are posted: without a first line of NO_REPLY, and empty, so not posted,
/// when that is all they are.
pub fn posted(words: &str) -> &str {
    match start(words) {
        Start::NoReply(rest) => rest.trim_start(),
        Start::Undecided if no_reply(words) => "",
        _ => words,
    }
}

/// An agent's saved words: without REPLY, and with what they answer when they start with it. That
/// is the teammate's message that woke the turn, the latest from another agent that reached it, or,
/// when that one is itself a reply, the message it sits under. Woken by the person, a REPLY is a
/// main message.
pub fn words<'a>(
    conn: &Connection,
    turn: &str,
    words: &'a str,
) -> Result<(&'a str, Option<String>), Error> {
    let rest = match start(words) {
        Start::Reply(rest) => rest,
        Start::Undecided if words.trim() == REPLY => "",
        _ => return Ok((words, None)),
    };
    Ok((rest.trim_start(), reply_to(conn, turn)?))
}

/// The main message a REPLY in `turn` sits under (`words`): in a check turn or the lead's last word
/// the lead's message checked, else the teammate's that reached it, or none when no teammate's words
/// reached it.
pub fn reply_to(conn: &Connection, turn: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT coalesce(
                 (SELECT c.answer FROM turns u JOIN open_checks c ON c.answer_turn = u.answers
                  WHERE u.id = ?1 AND u.opened_by IN ('check', 'last_word') ORDER BY c.id DESC LIMIT 1),
                 (SELECT coalesce(o.reply_to, o.id) FROM messages c
                  JOIN messages o ON o.id = c.copy_of JOIN turns ot ON ot.id = o.turn_id
                  JOIN turns me ON me.id = c.turn_id
                  WHERE c.turn_id = ?1 AND o.role = 'assistant' AND ot.agent_id IS NOT me.agent_id
                  ORDER BY c.ts DESC, c.rowid DESC LIMIT 1))";
    let found = conn.prepare_cached(sql)?.query_row([turn], |r| r.get(0));
    Ok(found?)
}

/// SQL for a message `m` said in the room, so not an agent's working note: in a turn only its last
/// block of words is said, once the turn has ended; the blocks before it are notes, kept in its
/// steps. Empty last words, which NO_REPLY after notes leaves, say nothing.
pub const SAID: &str = "(m.role != 'assistant' OR (trim(m.body) != ''
    AND EXISTS (SELECT 1 FROM turns e WHERE e.id = m.turn_id AND e.ended_ts IS NOT NULL)
    AND NOT EXISTS (
    SELECT 1 FROM messages n WHERE n.chat_id = m.chat_id AND n.ts >= m.ts AND n.turn_id = m.turn_id
      AND n.role = 'assistant' AND n.kind = 'text' AND n.rowid != m.rowid
      AND (n.ts > m.ts OR n.rowid > m.rowid))))";

/// What `turn` said (`SAID`): its last block of words, with its id. `None` when it wrote none, or
/// ended on NO_REPLY.
pub fn last_words(conn: &Connection, turn: &str) -> Result<Option<(String, String)>, Error> {
    let sql = "SELECT id, body FROM messages WHERE turn_id = ?1 AND role = 'assistant'
               AND kind = 'text' ORDER BY ts DESC, rowid DESC LIMIT 1";
    let found: Option<(String, String)> = conn
        .prepare_cached(sql)?
        .query_row([turn], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(found.filter(|(_, body)| !body.trim().is_empty()))
}

/// The most lines a turn hears.
pub const MOST: usize = 30;
/// The most a line cut to its first sentence keeps, in characters.
const GIST: usize = 160;

const FRESH: &str = "Recent lines in this chat:";
const SINCE: &str = "New in this chat since your last turn:";
const CUT: &str = "(Lines not for you are cut to their first sentence: read_chat has them whole.)";

/// What a turn hears, and what that costs.
#[derive(Debug, PartialEq)]
pub struct Heard {
    pub text: String,
    pub lines: usize,
    /// Lines cut to their first sentence.
    pub cut: usize,
    /// The last 30 lines, as a new conversation hears them.
    pub fresh: bool,
}

struct Line {
    id: String,
    role: String,
    body: String,
    agent: Option<String>,
    name: Option<String>,
}

/// The agent's started turns in `chat` but `turn`.
const MINE: &str = "WITH mine AS (SELECT u.id, u.created_ts FROM turns u
    WHERE u.chat_id = ?1 AND u.id != ?2 AND u.agent_id = ?3 AND u.position IS NOT NULL) ";

/// When `agent` last ran a turn in `chat` before `turn`. `None` when it never did.
pub fn last_ran(
    conn: &Connection,
    chat: &str,
    turn: &str,
    agent: &str,
) -> Result<Option<i64>, Error> {
    let sql = format!("{MINE}SELECT max(created_ts) FROM mine");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row(params![chat, turn, agent], |r| r.get(0))?)
}

/// What `agent` hears as `turn` opens in `chat`: what came since `since` (`last_ran`), or the last
/// lines with none. `None` in a chat of one agent, for an agent not in the chat, or with nothing to
/// hear.
pub fn catch_up(
    conn: &Connection,
    chat: &str,
    (turn, agent): (&str, &str),
    since: Option<i64>,
) -> Result<Option<Heard>, Error> {
    let team = team::members(conn, chat)?;
    let Some(me) = team
        .iter()
        .find(|m| m.agent == agent)
        .filter(|_| team.len() > 1)
    else {
        return Ok(None);
    };
    let mut lines = lines(conn, (chat, turn, agent), since)?;
    lines.reverse();
    if lines.is_empty() {
        return Ok(None);
    }
    let lead = team.iter().find(|m| m.role == Role::Supervisor);
    let mut cut = 0;
    let mut said = vec![match since {
        Some(_) => SINCE.to_string(),
        None => FRESH.to_string(),
    }];
    for line in &lines {
        let words = match whole(line, me, lead, &team) {
            true => None,
            false => gist(&line.body),
        };
        cut += usize::from(words.is_some());
        let words = words.unwrap_or_else(|| text::kept(&line.body, &line.id));
        said.push(format!("{}: {words}", speaker(line)));
    }
    if cut > 0 {
        said.push(CUT.to_string());
    }
    Ok(Some(Heard {
        text: said.join("\n"),
        lines: lines.len(),
        cut,
        fresh: since.is_none(),
    }))
}

/// The chat's lines, newest first, the last 30: since `since` (an agent's words when its turn ended
/// since, as they weren't said before) without the conversation's own turns
/// and what they carried, or the last of all. Never the turn's own messages or what they carry, nor
/// what the open check's words hand the turn already (0080): a check its answer and the person's
/// message, the lead's last word the members' checks.
fn lines(
    conn: &Connection,
    (chat, turn, agent): (&str, &str, &str),
    since: Option<i64>,
) -> Result<Vec<Line>, Error> {
    // A teammate's question to the person, as Gathr's line at the time of its answer (open: when
    // asked). Its asker hears the answer as its turn's ask instead.
    let sql = format!(
        "{MINE}SELECT 'question ' || q.id AS id, 'gathr', coalesce(a.name, 'An agent')
             || ' asked the person \"' || q.question || '\". ' || CASE WHEN q.answered_ts IS NULL
             THEN 'Not answered yet.' ELSE 'They answered: '
             || concat_ws('. ', 'Picked: ' || q.pick, 'Their words: ' || q.words) END,
             NULL, NULL, coalesce(q.answered_ts, q.asked_ts) AS ts
         FROM questions q LEFT JOIN agents a ON a.id = q.agent
         WHERE q.chat_id = ?1 AND q.agent != ?3 AND (?4 IS NULL OR coalesce(q.answered_ts, q.asked_ts) > ?4)
         UNION ALL
         SELECT m.id, m.role, m.body, t.agent_id, a.name, m.ts FROM messages m
         LEFT JOIN turns t ON t.id = m.turn_id LEFT JOIN agents a ON a.id = t.agent_id
         WHERE m.chat_id = ?1 AND m.kind = 'text' AND m.copy_of IS NULL
           AND (m.role IN ('user', 'assistant') OR (m.role = 'gathr' AND m.turn_id IS NULL))
           AND {SAID} AND m.turn_id IS NOT ?2
           AND m.id NOT IN (SELECT copy_of FROM messages WHERE turn_id = ?2 AND copy_of IS NOT NULL)
           AND NOT EXISTS (SELECT 1 FROM turns u JOIN open_checks c ON c.answer_turn = u.answers
                           WHERE u.id = ?2 AND u.opened_by = 'check' AND m.id IN (c.asked, c.answer))
           AND NOT EXISTS (SELECT 1 FROM turns u JOIN turns k ON k.answers = u.answers
                           WHERE u.id = ?2 AND u.opened_by = 'last_word' AND k.opened_by = 'check'
                             AND k.id = m.turn_id)
           AND (?4 IS NULL OR ((m.ts > ?4 OR (m.role = 'assistant' AND t.ended_ts > ?4))
               AND (m.turn_id IS NULL OR m.turn_id NOT IN (SELECT id FROM mine))
               AND m.id NOT IN (SELECT c.copy_of FROM messages c JOIN mine u ON u.id = c.turn_id
                                WHERE c.copy_of IS NOT NULL)))
         ORDER BY ts DESC, id DESC LIMIT ?5"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![chat, turn, agent, since, MOST as i64], |r| {
        Ok(Line {
            id: r.get(0)?,
            role: r.get(1)?,
            body: r.get(2)?,
            agent: r.get(3)?,
            name: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Whether a line is heard whole: by the lead every line; by a member the person's, Gathr's, the
/// lead's, its own and one that names it.
fn whole(line: &Line, me: &Member, lead: Option<&Member>, team: &[Member]) -> bool {
    let by = line.agent.as_deref();
    me.role == Role::Supervisor
        || line.role != "assistant"
        || by == Some(me.agent.as_str())
        || by.is_some_and(|by| lead.is_some_and(|l| l.agent == by))
        || team::mentioned(team, &line.body)
            .iter()
            .any(|m| m.agent == me.agent)
}

fn speaker(line: &Line) -> &str {
    match line.role.as_str() {
        "user" => "Person",
        "gathr" => "Gathr",
        _ => line.name.as_deref().unwrap_or("An agent"),
    }
}

/// A line cut to its first sentence, at most 160 characters, ending "…"; `None` when that is all
/// of it.
fn gist(body: &str) -> Option<String> {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = flat.chars().collect();
    let end = chars
        .iter()
        .enumerate()
        .find(|(at, c)| {
            matches!(c, '.' | '!' | '?') && chars.get(at + 1).is_none_or(|n| n.is_whitespace())
        })
        .map_or(chars.len(), |(at, _)| at + 1);
    let sentence: String = chars[..end].iter().collect();
    let short = text::cut(&sentence, GIST, false);
    if short.chars().count() >= chars.len() {
        return None;
    }
    Some(match short.ends_with('…') {
        true => short,
        false => format!("{short} …"),
    })
}

#[cfg(test)]
mod tests {
    use super::gist;

    #[test]
    fn a_line_is_cut_to_its_first_sentence_or_160_characters() {
        assert_eq!(
            gist("The map is wrong. The river bends."),
            Some("The map is wrong. …".into())
        );
        assert_eq!(gist("Done!\n\nAll of it."), Some("Done! …".into()));
        assert_eq!(gist("One sentence only."), None);
        assert_eq!(gist("v1.2 is out, no stop"), None);
        let long = format!("{} and more.", "word ".repeat(40));
        let cut = gist(&long).unwrap();
        assert_eq!(cut.chars().count(), 160, "{cut}");
        assert!(cut.ends_with('…'), "{cut}");
    }
}
