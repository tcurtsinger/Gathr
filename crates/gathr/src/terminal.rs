//! Terminals beside the chat: shells the service runs on a pseudo console (ConPTY on Windows), so
//! they keep running while the window is closed and every window, and later the chat's agents, see
//! the same one. Each belongs to a chat. What one printed is kept, and saved under the data folder
//! with its tab, so after a restart the tab and its output come back, ended, to run again.

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::Error;
use crate::ids::{now_us, ulid};
use crate::platform::env::Vars;

/// What is kept of what a terminal printed: its last bytes.
const KEPT: usize = 1 << 20;
/// Running terminals an agent may have started in one chat.
pub const AGENT_MOST: usize = 5;

/// A terminal as the window and the agents see it.
#[derive(Clone, Serialize, Deserialize)]
pub struct Info {
    pub id: String,
    pub chat: String,
    /// The tab's name: "Terminal", or the command an agent ran.
    pub name: String,
    pub folder: PathBuf,
    /// The agent that started it; none for the person's.
    pub agent: Option<String>,
    /// The command it runs in place of a shell the person types in.
    pub command: Option<String>,
    pub running: bool,
    /// How it ended, when it told.
    pub code: Option<u32>,
    pub created_us: i64,
}

/// What changed, for the windows showing a chat: its list of terminals, or a terminal's output.
#[derive(Clone)]
pub enum Out {
    Listed(Arc<str>),
    Printed(Arc<str>, Arc<str>, Arc<[u8]>),
}

struct Term {
    info: Info,
    kept: VecDeque<u8>,
    size: (u16, u16),
    /// While it runs.
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    killer: Option<Box<dyn ChildKiller + Send + Sync>>,
    /// Stop ended it, so how it ended says nothing.
    stopped: bool,
}

/// Every terminal, shared by the window's sockets and the agents' tools. Cloning shares them.
#[derive(Clone)]
pub struct Terminals {
    dir: PathBuf,
    terms: Arc<Mutex<HashMap<String, Term>>>,
    out: broadcast::Sender<Out>,
}

impl Terminals {
    /// The terminals saved in `dir`, each ended: none runs until asked again.
    pub fn load(dir: PathBuf) -> Terminals {
        let mut terms = HashMap::new();
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let read = std::fs::read(&path).ok();
            let Some(mut info) = read.and_then(|b| serde_json::from_slice::<Info>(&b).ok()) else {
                continue;
            };
            info.running = false;
            let kept = std::fs::read(path.with_extension("log")).unwrap_or_default();
            let term = Term {
                info,
                kept: kept.into(),
                size: (80, 24),
                writer: None,
                master: None,
                killer: None,
                stopped: false,
            };
            terms.insert(term.info.id.clone(), term);
        }
        Terminals {
            dir,
            terms: Arc::new(Mutex::new(terms)),
            out: broadcast::channel(256).0,
        }
    }

    pub fn watch(&self) -> broadcast::Receiver<Out> {
        self.out.subscribe()
    }

    /// The chat's terminals, oldest first.
    pub fn list(&self, chat: &str) -> Vec<Info> {
        let terms = self.terms.lock().unwrap();
        let mut list: Vec<Info> = terms
            .values()
            .filter(|t| t.info.chat == chat)
            .map(|t| t.info.clone())
            .collect();
        list.sort_by_key(|i| i.created_us);
        list
    }

    pub fn info(&self, id: &str) -> Option<Info> {
        self.terms.lock().unwrap().get(id).map(|t| t.info.clone())
    }

    /// What it printed and still keeps.
    pub fn kept(&self, id: &str) -> Vec<u8> {
        let terms = self.terms.lock().unwrap();
        let kept = terms.get(id).map(|t| t.kept.iter().copied().collect());
        kept.unwrap_or_default()
    }

    /// Starts a terminal in `folder`: a shell, or `command` in one. Its id.
    pub fn open(
        &self,
        chat: &str,
        folder: &Path,
        agent: Option<&str>,
        command: Option<&str>,
        vars: Vars,
    ) -> Result<String, Error> {
        if let Some(agent) = agent {
            let terms = self.terms.lock().unwrap();
            let theirs = terms.values().filter(|t| {
                t.info.chat == chat && t.info.running && t.info.agent.as_deref() == Some(agent)
            });
            if theirs.count() >= AGENT_MOST {
                let why = format!("{AGENT_MOST} terminals of yours already run in this chat");
                return Err(Error::Refused(why));
            }
        }
        let now = now_us();
        let info = Info {
            id: ulid(now),
            chat: chat.into(),
            name: command.unwrap_or("Terminal").into(),
            folder: folder.into(),
            agent: agent.map(Into::into),
            command: command.map(Into::into),
            running: false,
            code: None,
            created_us: now,
        };
        let id = info.id.clone();
        let term = Term {
            info,
            kept: VecDeque::new(),
            size: (80, 24),
            writer: None,
            master: None,
            killer: None,
            stopped: false,
        };
        self.terms.lock().unwrap().insert(id.clone(), term);
        if let Err(e) = self.start(&id, vars) {
            self.terms.lock().unwrap().remove(&id);
            return Err(e);
        }
        Ok(id)
    }

    /// Runs an ended terminal again in its tab, under what it printed before.
    pub fn again(&self, id: &str, vars: Vars) -> Result<(), Error> {
        let running = self.info(id).map(|i| i.running);
        match running {
            None => Err(Error::Refused("No such terminal".into())),
            Some(true) => Ok(()),
            Some(false) => self.start(id, vars),
        }
    }

    fn start(&self, id: &str, vars: Vars) -> Result<(), Error> {
        let (folder, command, size) = {
            let terms = self.terms.lock().unwrap();
            let t = terms
                .get(id)
                .ok_or_else(|| Error::Other("No such terminal".into()))?;
            (t.info.folder.clone(), t.info.command.clone(), t.size)
        };
        let failed = |e: Box<dyn std::error::Error>| Error::Other(e.to_string());
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                cols: size.0,
                rows: size.1,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| failed(e.into()))?;
        let mut cmd = shell(&vars, command.as_deref());
        cmd.cwd(&folder);
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| failed(e.into()))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| failed(e.into()))?;
        let writer = pair.master.take_writer().map_err(|e| failed(e.into()))?;
        {
            let mut terms = self.terms.lock().unwrap();
            let t = terms
                .get_mut(id)
                .ok_or_else(|| Error::Other("No such terminal".into()))?;
            t.killer = Some(child.clone_killer());
            t.writer = Some(writer);
            t.master = Some(pair.master);
            t.info.running = true;
            t.info.code = None;
            t.stopped = false;
            self.save(&t.info, None);
            let _ = self.out.send(Out::Listed(t.info.chat.as_str().into()));
        }
        let me = self.clone();
        let read_id = id.to_string();
        std::thread::spawn(move || me.read(&read_id, reader));
        let me = self.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code());
            me.ended(&id, code);
        });
        Ok(())
    }

    fn read(&self, id: &str, mut reader: Box<dyn Read + Send>) {
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            let mut terms = self.terms.lock().unwrap();
            let Some(t) = terms.get_mut(id) else { break };
            t.kept.extend(&buf[..n]);
            let over = t.kept.len().saturating_sub(KEPT);
            t.kept.drain(..over);
            let chat: Arc<str> = t.info.chat.as_str().into();
            let _ = self
                .out
                .send(Out::Printed(chat, id.into(), buf[..n].into()));
        }
    }

    fn ended(&self, id: &str, code: Option<u32>) {
        let mut terms = self.terms.lock().unwrap();
        let Some(t) = terms.get_mut(id) else { return };
        // Closing the console ends the reader on Windows, where the shell's end doesn't.
        t.writer = None;
        t.master = None;
        t.killer = None;
        t.info.running = false;
        t.info.code = code.filter(|_| !t.stopped);
        self.save(&t.info, Some(&t.kept));
        let _ = self.out.send(Out::Listed(t.info.chat.as_str().into()));
    }

    /// Keys the person, or an agent, typed.
    pub fn input(&self, id: &str, bytes: &[u8]) -> Result<(), Error> {
        let mut terms = self.terms.lock().unwrap();
        let writer = terms.get_mut(id).and_then(|t| t.writer.as_mut());
        let writer = writer.ok_or_else(|| Error::Refused("That terminal has ended".into()))?;
        writer.write_all(bytes)?;
        Ok(writer.flush()?)
    }

    /// The size the window shows it at. The last window to say wins.
    pub fn resize(&self, id: &str, cols: u16, rows: u16) {
        let (cols, rows) = (cols.clamp(2, 1000), rows.clamp(1, 500));
        let mut terms = self.terms.lock().unwrap();
        let Some(t) = terms.get_mut(id) else { return };
        t.size = (cols, rows);
        if let Some(master) = &t.master {
            let size = PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            };
            let _ = master.resize(size);
        }
    }

    /// Ends what it runs; the tab stays, with its output.
    pub fn stop(&self, id: &str) {
        let mut terms = self.terms.lock().unwrap();
        let Some(t) = terms.get_mut(id) else { return };
        if let Some(killer) = t.killer.as_mut() {
            t.stopped = true;
            let _ = killer.kill();
        }
    }

    /// Ends it and forgets it, tab and output.
    pub fn close(&self, id: &str) {
        self.stop(id);
        let Some(t) = self.terms.lock().unwrap().remove(id) else {
            return;
        };
        let _ = std::fs::remove_file(self.dir.join(format!("{id}.json")));
        let _ = std::fs::remove_file(self.dir.join(format!("{id}.log")));
        let _ = self.out.send(Out::Listed(t.info.chat.as_str().into()));
    }

    /// Ends what a chat being archived runs.
    pub fn stop_chat(&self, chat: &str) {
        for info in self.list(chat).iter().filter(|i| i.running) {
            self.stop(&info.id);
        }
    }

    /// Every terminal of a chat being deleted.
    pub fn close_chat(&self, chat: &str) {
        for info in self.list(chat) {
            self.close(&info.id);
        }
    }

    /// The service is stopping: what each printed is saved, then each ends.
    pub fn stop_all(&self) {
        let ids: Vec<String> = {
            let terms = self.terms.lock().unwrap();
            for t in terms.values().filter(|t| t.info.running) {
                self.save(&t.info, Some(&t.kept));
            }
            terms.keys().cloned().collect()
        };
        for id in ids {
            self.stop(&id);
        }
    }

    fn save(&self, info: &Info, kept: Option<&VecDeque<u8>>) {
        let _ = std::fs::create_dir_all(&self.dir);
        let path = self.dir.join(format!("{}.json", info.id));
        if let Ok(json) = serde_json::to_vec(info) {
            let _ = std::fs::write(&path, json);
        }
        if let Some(kept) = kept {
            let (a, b) = kept.as_slices();
            let _ = std::fs::write(path.with_extension("log"), [a, b].concat());
        }
    }
}

/// The person's shell, or `command` run by it: on Windows PowerShell 7 where it's installed, else
/// Windows PowerShell; elsewhere the login shell.
fn shell(vars: &Vars, command: Option<&str>) -> CommandBuilder {
    let var = |name: &str| {
        let found = vars.iter().find(|(k, _)| k.eq_ignore_ascii_case(name));
        found.map(|(_, v)| v.clone())
    };
    let mut cmd = if cfg!(windows) {
        let path = var("PATH").unwrap_or_default();
        let pwsh = crate::platform::find_program("pwsh", &path);
        let program = pwsh.unwrap_or_else(|| PathBuf::from("powershell.exe"));
        let mut cmd = CommandBuilder::new(program);
        cmd.arg("-NoLogo");
        if let Some(command) = command {
            cmd.args(["-Command", command]);
        }
        cmd
    } else {
        let program = var("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
        let mut cmd = CommandBuilder::new(program);
        cmd.arg("-l");
        if let Some(command) = command {
            cmd.args(["-c", command]);
        }
        cmd
    };
    cmd.env_clear();
    for (k, v) in vars {
        cmd.env(k, v);
    }
    if !cfg!(windows) {
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
    }
    cmd
}
