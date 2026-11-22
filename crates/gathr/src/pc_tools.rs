//! Tools on this PC: a local MCP server, a program Gathr starts itself and talks to over its stdin and
//! stdout, served to engines through the same relay as a tool on the web (`web_tools`), so an engine
//! only ever talks to Gathr and one copy serves every agent. Gathr reads what a person pastes from a
//! tool's instructions, says back what it would run before anything runs, and keeps the tool's
//! variables sealed as a key is: their values reach only the program.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::transport::TokioChildProcess;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

use crate::ids::{now_us, ulid};
use crate::platform::env::Vars;
use crate::web_tools::{self, Remote, Tool};
use crate::{Error, seal};

/// How long a program has to start and answer Gathr's hello: a package manager may fetch it first.
pub const START: Duration = Duration::from_secs(60);
/// What an agent's call to a tool that won't start is told.
pub const NOT_STARTED: &str = "The tool didn't start";
const UNREADABLE: &str = "Gathr can't read this as a command or JSON.";
/// What is kept of what a program printed: its last lines, within this many lines and bytes.
const PRINTED_LINES: usize = 40;
const PRINTED_BYTES: usize = 8 * 1024;

/// What a tool on this PC runs: the program as typed, its arguments, the folder it runs in if one is
/// given, and its variables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Runs {
    pub program: String,
    pub args: Vec<String>,
    pub folder: Option<String>,
    pub variables: BTreeMap<String, String>,
}

/// What was pasted: the tool's name (its JSON key, else its program's) and what it runs.
#[derive(Debug, PartialEq)]
pub struct Pasted {
    pub name: String,
    pub runs: Runs,
}

/// Reads a command line, or a JSON block in one of the shapes tools' instructions use: `mcpServers`
/// or `servers` naming one tool, the tool by its name alone, or the tool's own `command`.
pub fn read(text: &str) -> Result<Pasted, Error> {
    let unreadable = || Error::Refused(UNREADABLE.into());
    let text = text.trim();
    if text.starts_with('{') {
        let value: Value = serde_json::from_str(text).map_err(|_| unreadable())?;
        return from_json(value.as_object().ok_or_else(unreadable)?);
    }
    let words = split(text).ok_or_else(unreadable)?;
    let (program, args) = words.split_first().ok_or_else(unreadable)?;
    let runs = Runs {
        program: program.clone(),
        args: args.to_vec(),
        ..Runs::default()
    };
    Ok(Pasted {
        name: stem(program),
        runs,
    })
}

fn from_json(object: &Map<String, Value>) -> Result<Pasted, Error> {
    let unreadable = || Error::Refused(UNREADABLE.into());
    if object.contains_key("command") {
        let runs = server(object).ok_or_else(unreadable)?;
        let name = stem(&runs.program);
        return Ok(Pasted { name, runs });
    }
    let servers = ["mcpServers", "servers"]
        .iter()
        .find_map(|key| object.get(*key)?.as_object())
        .unwrap_or(object);
    let mut entries = servers.iter();
    let (Some((name, entry)), None) = (entries.next(), entries.next()) else {
        return Err(match servers.len() {
            0 => unreadable(),
            n => Error::Refused(format!("This has {n} tools. Paste one at a time.")),
        });
    };
    let runs = entry.as_object().and_then(server).ok_or_else(unreadable)?;
    let name = name.clone();
    Ok(Pasted { name, runs })
}

/// One tool's `command`, `args`, `env` and `cwd`.
fn server(entry: &Map<String, Value>) -> Option<Runs> {
    let program = entry.get("command")?.as_str()?.trim().to_string();
    let args = match entry.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(args)) => args.iter().map(word).collect::<Option<_>>()?,
        Some(_) => return None,
    };
    let variables = match entry.get("env") {
        None | Some(Value::Null) => BTreeMap::new(),
        Some(Value::Object(env)) => env
            .iter()
            .map(|(name, value)| Some((name.clone(), word(value)?)))
            .collect::<Option<_>>()?,
        Some(_) => return None,
    };
    let folder = match entry.get("cwd") {
        None | Some(Value::Null) => None,
        Some(Value::String(folder)) => Some(folder.trim().to_string()).filter(|f| !f.is_empty()),
        Some(_) => return None,
    };
    let runs = Runs {
        program,
        args,
        folder,
        variables,
    };
    (!runs.program.is_empty()).then_some(runs)
}

fn word(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

/// A command line's words: split on spaces, double or single quotes grouping. A backslash is a plain
/// character, as in a Windows path. None when a quote isn't closed.
fn split(text: &str) -> Option<Vec<String>> {
    let (mut words, mut word, mut started, mut quote) = (Vec::new(), String::new(), false, None);
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                }
                started = false;
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

/// A program's file name without its extension: `npx` for `C:\nodejs\npx.cmd`.
fn stem(program: &str) -> String {
    let file = program.rsplit(['/', '\\']).next().unwrap_or(program);
    match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => file.to_string(),
    }
}

/// Where a program run through a package runner comes from: the registry and the package.
pub fn from(runs: &Runs) -> Option<String> {
    let registry = match stem(&runs.program).to_ascii_lowercase().as_str() {
        "npx" | "bunx" => "npm",
        "uvx" => "PyPI",
        _ => return None,
    };
    let package = runs.args.iter().find(|a| !a.starts_with('-'))?;
    Some(format!("{registry}, the package {package}"))
}

/// What a person is told when the program isn't found. Gathr installs nothing.
pub fn missing(typed: &str) -> String {
    let comes = match stem(typed).to_ascii_lowercase().as_str() {
        "node" | "npx" | "npm" => " It comes with Node.js.",
        "uv" | "uvx" => " It comes with uv.",
        "python" | "python3" => " It comes with Python.",
        _ => "",
    };
    format!("{typed} isn’t on this PC.{comes}")
}

/// The program's full path, as the engines' environment `vars` finds it: a path as it is if the file
/// is there, a bare name on its PATH.
pub fn find(typed: &str, vars: &Vars) -> Option<PathBuf> {
    if typed.contains(['/', '\\']) {
        let path = Path::new(typed);
        return path.is_file().then(|| path.to_path_buf());
    }
    let path = var(vars, "PATH")?;
    crate::platform::find_program(typed, std::ffi::OsStr::new(&path))
}

/// A variable's value; names ignore case on Windows.
fn var(vars: &Vars, name: &str) -> Option<String> {
    let found = vars.iter().find(|(n, _)| match cfg!(windows) {
        true => n.to_string_lossy().eq_ignore_ascii_case(name),
        false => n == name,
    });
    found.map(|(_, value)| value.to_string_lossy().into_owned())
}

/// What a tool's variables are sealed under, beside its key's and its sign-in's names.
pub fn sealed_as(id: &str) -> String {
    format!("{id} variables")
}

/// What is stored of `runs`: its arguments as JSON, and its variables sealed, or none.
fn stored(id: &str, runs: &Runs) -> Result<(String, Option<Vec<u8>>), Error> {
    let json = |e: serde_json::Error| Error::Other(e.to_string());
    let args = serde_json::to_string(&runs.args).map_err(json)?;
    if runs.variables.is_empty() {
        return Ok((args, None));
    }
    let variables = serde_json::to_string(&runs.variables).map_err(json)?;
    Ok((args, Some(seal::seal(&sealed_as(id), &variables)?)))
}

/// Saves a tool on this PC, its variables sealed. Its link is empty: it has none.
pub fn add(conn: &Connection, name: &str, runs: &Runs) -> Result<Tool, Error> {
    let id = ulid(now_us());
    let (args, variables) = stored(&id, runs)?;
    conn.execute(
        "INSERT INTO tools (id, name, link, enabled, added_ts, program, args, folder, variables)
         VALUES (?1, ?2, '', 1, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            name,
            now_us(),
            runs.program,
            args,
            runs.folder,
            variables
        ],
    )?;
    web_tools::get(conn, &id)
}

/// Replaces what the tool runs, and its name when one is given.
pub fn change(conn: &Connection, id: &str, name: Option<&str>, runs: &Runs) -> Result<Tool, Error> {
    saved(conn, id)?;
    let (args, variables) = stored(id, runs)?;
    conn.execute(
        "UPDATE tools SET name = COALESCE(?2, name), program = ?3, args = ?4, folder = ?5,
         variables = ?6 WHERE id = ?1",
        params![id, name, runs.program, args, runs.folder, variables],
    )?;
    web_tools::get(conn, id)
}

/// What the tool runs, its variables unsealed: for starting it, and their names for an answer.
pub fn saved(conn: &Connection, id: &str) -> Result<Runs, Error> {
    type Row = (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Vec<u8>>,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT program, args, folder, variables FROM tools WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((program, args, folder, variables)) = row else {
        return Err(Error::Refused(format!("There's no tool {id}")));
    };
    let program = program.ok_or_else(|| Error::Refused(format!("{id} isn't on this PC")))?;
    let json = |e: serde_json::Error| Error::Other(format!("{id}'s command wasn't read: {e}"));
    let args = serde_json::from_str(args.as_deref().unwrap_or("[]")).map_err(json)?;
    let variables = match variables {
        Some(sealed) => {
            serde_json::from_str(&seal::unseal(&sealed_as(id), &sealed)?).map_err(json)?
        }
        None => BTreeMap::new(),
    };
    Ok(Runs {
        program,
        args,
        folder,
        variables,
    })
}

/// A change's `runs`, where a variable given with no value keeps its value in `was`.
pub fn kept(mut runs: Runs, was: &Runs) -> Runs {
    for (name, value) in &mut runs.variables {
        if let (true, Some(old)) = (value.is_empty(), was.variables.get(name)) {
            value.clone_from(old);
        }
    }
    runs
}

/// The last lines a program printed on its stderr, with its variables' values hidden.
#[derive(Default)]
pub struct Printed {
    lines: VecDeque<String>,
    bytes: usize,
}

impl Printed {
    pub fn push(&mut self, line: &str) {
        let mut line = line.trim_end().to_string();
        if line.is_empty() {
            return;
        }
        if line.len() > PRINTED_BYTES {
            let mut end = PRINTED_BYTES;
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            line.truncate(end);
        }
        self.bytes += line.len();
        self.lines.push_back(line);
        while self.lines.len() > PRINTED_LINES || self.bytes > PRINTED_BYTES {
            let gone = self.lines.pop_front().map_or(0, |l| l.len());
            self.bytes -= gone;
        }
    }

    /// What it printed, or none.
    pub fn text(&self) -> Option<String> {
        let lines: Vec<&str> = self.lines.iter().map(String::as_str).collect();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

/// The environment a tool's program gets: the engines' (`vars`) without Gathr's own engine token
/// and push address, and the tool's variables over it.
fn environment(vars: Vars, runs: &Runs) -> Vars {
    use crate::claude::command::{PUSH_VAR, TOKEN_VAR};
    let ours = |name: &std::ffi::OsStr| {
        let name = name.to_string_lossy();
        [TOKEN_VAR, PUSH_VAR]
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&name))
    };
    let mut env: Vars = vars.into_iter().filter(|(name, _)| !ours(name)).collect();
    env.extend(runs.variables.iter().map(|(k, v)| (k.into(), v.into())));
    env
}

/// Starts `program` (found for `runs`) with the engines' environment `vars`, and connects to it
/// over its stdin and stdout within `START`: none when it didn't start. No console window; it ends with everything it started
/// when dropped, and on Windows when gathrd ends. What it prints on stderr goes to `printed` only.
pub async fn start(
    runs: &Runs,
    program: &Path,
    vars: Vars,
    printed: Arc<Mutex<Printed>>,
) -> Option<Remote> {
    let home = var(&vars, if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let folder = runs.folder.clone().or(home).unwrap_or_else(|| ".".into());
    let mut command = tokio::process::Command::new(program);
    command
        .args(&runs.args)
        .current_dir(folder)
        .env_clear()
        .envs(environment(vars, runs));
    let mut wrapped = crate::platform::ending_together(command);
    // On Windows the job then ends the program, and what it started, when gathrd ends.
    wrapped.wrap(process_wrap::tokio::KillOnDrop);
    let builder = TokioChildProcess::builder(wrapped).stderr(Stdio::piped());
    let (transport, stderr) = match builder.spawn() {
        Ok(spawned) => spawned,
        Err(e) => {
            lock(&printed).push(&format!("It didn't start: {e}"));
            return None;
        }
    };
    let hidden: Vec<String> = runs.variables.values().cloned().collect();
    let reading = stderr.map(|stderr| tokio::spawn(read_printed(stderr, printed, hidden)));
    match tokio::time::timeout(START, ().serve(transport)).await {
        Ok(Ok(remote)) => Some(remote),
        _ => {
            // What it printed as it ended is read before it's shown.
            if let Some(reading) = reading {
                let _ = tokio::time::timeout(Duration::from_secs(2), reading).await;
            }
            None
        }
    }
}

fn lock(printed: &Mutex<Printed>) -> std::sync::MutexGuard<'_, Printed> {
    printed.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads the program's stderr as it comes, for as long as it runs, so it never fills.
async fn read_printed(
    stderr: tokio::process::ChildStderr,
    printed: Arc<Mutex<Printed>>,
    hidden: Vec<String>,
) {
    let mut reader = tokio::io::BufReader::new(stderr);
    let mut line = Vec::new();
    loop {
        line.clear();
        let limit = (PRINTED_BYTES + 1) as u64;
        match (&mut reader).take(limit).read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let mut text = String::from_utf8_lossy(&line).into_owned();
                for value in hidden.iter().filter(|v| v.len() >= 4) {
                    text = text.replace(value.as_str(), "••••");
                }
                lock(&printed).push(&text);
            }
        }
    }
}

#[cfg(test)]
mod tests;
