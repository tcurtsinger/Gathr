//! Made-up test data (docs/first-slice.md): a chat of 500 messages, a turn with 50 tool calls
//! and a 20,000-word answer, with their events, a five-agent team chat (F8b), and
//! questions for the person across two chats (F9). Nothing here
//! comes from a real conversation.

mod asks;
mod team;

use rusqlite::{Connection, params};
use serde_json::json;

use crate::Error;
use crate::event::Event;
use crate::ids::{now_us, span_id, trace_id, ulid};

const WORDS: &[&str] = &[
    "river", "stone", "the", "current", "bank", "of", "slow", "water", "carries", "silt", "toward",
    "a", "delta", "and", "reeds", "bend", "in", "cold", "light", "where", "herons", "wait",
    "beside", "gravel", "bars", "that", "shift", "each", "spring", "flood", "while", "maps", "lag",
    "behind", "every", "new", "channel", "cut", "through", "clay",
];
const TOOLS: &[&str] = &["Read", "Grep", "Edit", "Bash", "Glob", "Write"];
/// What a seeded step ran (0041): short, long enough to be cut, and more than one line.
const PATHS: &[&str] = &[
    "src/river/delta.rs",
    "docs/gravel-bars.md",
    "tests/spring_flood.rs",
];
const COMMANDS: &[&str] = &[
    "git status --short",
    "cargo nextest run --workspace --locked",
    "cargo fmt --all --check\ncargo clippy --workspace --all-targets --locked -- -D warnings",
    "rg --files-with-matches --glob '*.rs' --glob '!target' 'slow water carries silt toward a delta' \
     src/river src/stone src/current src/bank tests/herons tests/gravel docs/maps docs/channel docs/clay \
     docs/reeds docs/light docs/flood docs/spring docs/bars docs/shift",
];

/// What the seeded `tool`'s step kept, picked by `n`; Glob keeps nothing, as an unlisted tool does.
fn ran(tool: &str, n: usize) -> Option<crate::steps::StepInput> {
    let text = match tool {
        "Read" | "Edit" | "Write" => PATHS[n % PATHS.len()],
        "Bash" => COMMANDS[n % COMMANDS.len()],
        "Grep" => WORDS[n % WORDS.len()],
        _ => return None,
    };
    crate::steps::input(text, None)
}
const DAY_US: i64 = 86_400_000_000;

/// What a seed run made.
#[derive(Debug, serde::Serialize)]
pub struct Seeded {
    pub chats: u32,
    pub messages: u32,
    pub events: u32,
}

/// Adds the five chats in one transaction, and the files of the person's tasks under `files`.
/// Each run adds a new set.
pub fn seed(conn: &mut Connection, files: &std::path::Path) -> Result<Seeded, Error> {
    let tx = conn.transaction()?;
    let mut s = Seeder {
        conn: &tx,
        files,
        ts: now_us() - 7 * DAY_US,
        rng: 0x9e37_79b9_7f4a_7c15,
        made: Seeded::zero(),
    };
    s.long_chat()?;
    s.tool_heavy_chat()?;
    s.long_answer_chat()?;
    s.team_chat()?;
    s.asks()?;
    let made = s.made;
    tx.commit()?;
    Ok(made)
}

impl Seeded {
    fn zero() -> Self {
        Seeded {
            chats: 0,
            messages: 0,
            events: 0,
        }
    }
}

struct Seeder<'a> {
    conn: &'a Connection,
    files: &'a std::path::Path,
    ts: i64,
    rng: u64,
    made: Seeded,
}

/// One made-up turn: its chat, trace and span, so its messages and events link up.
struct Turn {
    chat: String,
    id: String,
    trace: String,
    span: String,
    started: i64,
}

impl Seeder<'_> {
    /// 500 messages: turns of a question, sometimes a thought and tool calls, and an answer.
    fn long_chat(&mut self) -> Result<(), Error> {
        let chat = self.chat("Seed: 500 messages")?;
        let mut left = 500;
        while left > 0 {
            let tools = if left > 2 && self.next().is_multiple_of(3) {
                (self.next() % 4 + 1).min(left - 2)
            } else {
                0
            };
            let answer_words = 30 + self.next() % 370;
            let question_words = 5 + self.next() % 35;
            let turn = self.turn(&chat, question_words)?;
            let thought = tools > 0 && left > tools + 3;
            if thought {
                let ms = 1_500 + self.next() % 30_000;
                self.thought(&turn, ms as i64)?;
            }
            for _ in 0..tools {
                self.tool_call(&turn)?;
            }
            left -= 1 + tools + u64::from(thought);
            if left > 0 {
                self.answer(&turn, answer_words as usize)?;
                left -= 1;
            }
        }
        Ok(())
    }

    fn tool_heavy_chat(&mut self) -> Result<(), Error> {
        let chat = self.chat("Seed: 50 tool calls in a turn")?;
        let turn = self.turn(&chat, 12)?;
        for n in 0..50 {
            // Thoughts between the calls: one too short to show (0042).
            match n {
                0 => self.thought(&turn, 9_000)?,
                20 => self.thought(&turn, 400)?,
                40 => self.thought(&turn, 72_000)?,
                _ => {}
            }
            self.tool_call(&turn)?;
        }
        self.answer(&turn, 120)
    }

    fn long_answer_chat(&mut self) -> Result<(), Error> {
        let chat = self.chat("Seed: 20,000-word answer")?;
        let turn = self.turn(&chat, 9)?;
        self.answer(&turn, 20_000)
    }

    fn chat(&mut self, title: &str) -> Result<String, Error> {
        let id = ulid(self.ts);
        self.conn.execute(
            "INSERT INTO chats (id, title, created_ts) VALUES (?1, ?2, ?3)",
            params![id, title, self.ts],
        )?;
        self.made.chats += 1;
        Ok(id)
    }

    /// Starts a turn with the person's question.
    fn turn(&mut self, chat: &str, question_words: u64) -> Result<Turn, Error> {
        self.ts += 60_000_000;
        let turn = Turn {
            chat: chat.into(),
            id: ulid(self.ts),
            trace: trace_id(),
            span: span_id(),
            started: self.ts,
        };
        let question = self.text(question_words as usize);
        self.message(&turn, "user", "text", &question)?;
        let start = self
            .event(&turn, "invoke_agent seed", "start")
            .attr("gen_ai.operation.name", "invoke_agent");
        start.insert(self.conn)?;
        self.made.events += 1;
        Ok(turn)
    }

    fn tool_call(&mut self, turn: &Turn) -> Result<(), Error> {
        let tool = TOOLS[(self.next() % TOOLS.len() as u64) as usize];
        let duration_us = 5_000 + (self.next() % 3_000_000) as i64;
        let failed = self.next().is_multiple_of(17);
        self.ts += duration_us;
        let mut body = json!({ "tool": tool, "status": if failed { "error" } else { "ok" }, "duration_ms": duration_us / 1000 });
        if let Some(input) = ran(tool, duration_us as usize) {
            body["input"] = json!(input);
        }
        self.message(turn, "assistant", "tool", &body.to_string())?;
        let mut event = self
            .event(turn, &format!("execute_tool {tool}"), "end")
            .attr("gen_ai.tool.name", tool);
        event.duration_us = Some(duration_us);
        event.parent_span_id = Some(turn.span.clone());
        event.span_id = Some(span_id());
        event = if failed {
            event.error("tool_error").attr("process.exit.code", 1)
        } else {
            event.ok()
        };
        event.insert(self.conn)?;
        self.made.events += 1;
        Ok(())
    }

    /// A thought that took `ms`, as the service keeps one (0042): its time and a count of tokens.
    fn thought(&mut self, turn: &Turn, ms: i64) -> Result<(), Error> {
        let body = json!({ "status": "done", "duration_ms": ms, "tokens": ms / 20 });
        self.message(turn, "assistant", "thinking", &body.to_string())?;
        self.ts += ms * 1_000;
        let mut event = self
            .event(turn, "gathr.turn.thinking", "point")
            .attr("gathr.thinking.tokens_estimate", ms / 20);
        event.duration_us = Some(ms * 1_000);
        event.ok().insert(self.conn)?;
        self.made.events += 1;
        Ok(())
    }

    /// Ends the turn with the agent's answer.
    fn answer(&mut self, turn: &Turn, words: usize) -> Result<(), Error> {
        self.ts += 2_000_000 + words as i64 * 20_000;
        let text = self.text(words);
        self.message(turn, "assistant", "text", &text)?;
        let mut end = self.event(turn, "invoke_agent seed", "end").ok();
        end.duration_us = Some(self.ts - turn.started);
        end = end
            .attr("gen_ai.usage.input_tokens", 2_000 + self.next() % 30_000)
            .attr("gen_ai.usage.output_tokens", words as u64 * 4 / 3)
            .attr("gen_ai.response.finish_reasons", json!(["end_turn"]));
        end.insert(self.conn)?;
        self.made.events += 1;
        Ok(())
    }

    fn message(&mut self, turn: &Turn, role: &str, kind: &str, body: &str) -> Result<(), Error> {
        self.ts += 1_000;
        self.conn.execute(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![ulid(self.ts), turn.chat, turn.id, self.ts, role, kind, body],
        )?;
        self.made.messages += 1;
        Ok(())
    }

    fn event(&self, turn: &Turn, name: &str, phase: &'static str) -> Event {
        let mut event = Event::point(name).trace(&turn.trace);
        event.ts = self.ts;
        event.phase = phase;
        event.span_id = Some(turn.span.clone());
        event.conversation_id = Some(turn.chat.clone());
        event.attr("gathr.seed", true)
    }

    /// `n` made-up words, in sentences.
    fn text(&mut self, n: usize) -> String {
        let mut out = String::with_capacity(n * 6);
        for i in 0..n {
            let word = WORDS[(self.next() % WORDS.len() as u64) as usize];
            if i > 0 {
                out.push(' ');
            }
            out.push_str(word);
            if i % 14 == 13 || i + 1 == n {
                out.push('.');
            }
        }
        out
    }

    /// xorshift64: the same data every run, with no dependency.
    fn next(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
}
