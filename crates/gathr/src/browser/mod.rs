//! The browser beside the chat: one Edge or Chrome for the person, run by the service without a
//! window and with a profile of its own under the data folder, so its sign-ins stay. Gathr drives it
//! over its debugging pipe, never a port any program on the PC could reach. The window's Browser tab
//! shows the page being shown live, its pointer and its own dialogs, and sends the person's mouse, keys and
//! answers back (`pages`).

mod cdp;
mod pages;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::event::Event;
use crate::writer::Writer;

pub use pages::Out;

/// The argument that makes `gathrd` the browser's relay on Windows (`relay`).
pub const RELAY: &str = "--browser-relay";
/// How long a stopping service waits for the browser to close by itself and write its cookies.
const CLOSING: Duration = Duration::from_secs(5);
/// The longest wait before a browser that keeps dying is started again.
const LONGEST_WAIT: Duration = Duration::from_secs(60);

/// The person's one browser, shared by every window that shows it. Cloning shares it.
#[derive(Clone)]
pub struct Browser {
    said: mpsc::UnboundedSender<Cmd>,
    out: broadcast::Sender<Out>,
    state: Arc<Mutex<Out>>,
    clients: Arc<AtomicU64>,
}

enum Cmd {
    /// A window's message.
    Said(u64, Value),
    /// A window closed its socket.
    Left(u64),
    Stop(oneshot::Sender<()>),
    /// The browser of this start ended, and how.
    Ended(u64, String),
    /// A page's title, address and history, as read again.
    Looked(pages::Looked),
    /// The person's mouse moved over a page, by its id, to this place.
    Moved(String, f64, f64),
    /// The pointer a page shows where the mouse is, as it answered.
    Pointed(String, Option<String>),
    /// A while after the shown page got its nth size: unless a newer one came, its picture is sent again.
    Settled(u64),
}

impl Browser {
    /// Nothing starts until a window needs the browser.
    pub fn new(profile: PathBuf, writer: Writer) -> Browser {
        let (said, cmds) = mpsc::unbounded_channel();
        let out = broadcast::channel(16).0;
        let state = Arc::new(Mutex::new(Out::Text(Arc::default())));
        let actor = pages::Actor::new(profile, writer, out.clone(), state.clone(), said.clone());
        tokio::spawn(actor.run(cmds));
        let clients = Arc::default();
        Browser {
            said,
            out,
            state,
            clients,
        }
    }

    /// A number for a window's socket, for what it says and when it leaves.
    pub fn client(&self) -> u64 {
        self.clients.fetch_add(1, Ordering::Relaxed)
    }

    /// The tabs' state as now, then everything sent to the windows from here on.
    pub fn watch(&self) -> (Out, broadcast::Receiver<Out>) {
        let out = self.out.subscribe();
        let state = self
            .state
            .lock()
            .map(|s| s.clone())
            .unwrap_or(Out::Text(Arc::default()));
        (state, out)
    }

    pub fn said(&self, client: u64, message: Value) {
        let _ = self.said.send(Cmd::Said(client, message));
    }

    pub fn left(&self, client: u64) {
        let _ = self.said.send(Cmd::Left(client));
    }

    /// Closes the browser, letting it write its profile first, and stops it if it doesn't close.
    pub async fn stop(&self) {
        let (done, stopped) = oneshot::channel();
        if self.said.send(Cmd::Stop(done)).is_ok() {
            let _ = stopped.await;
        }
    }
}

/// The browser on this computer: Edge on Windows; Chrome or Edge on a Mac (Safari can't be driven).
pub fn find() -> Option<PathBuf> {
    let found: Vec<PathBuf> = if cfg!(windows) {
        ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|base| PathBuf::from(base).join(r"Microsoft\Edge\Application\msedge.exe"))
            .collect()
    } else if cfg!(target_os = "macos") {
        let apps = [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];
        let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Applications"));
        [Some(PathBuf::from("/Applications")), home]
            .into_iter()
            .flatten()
            .flat_map(|folder| apps.map(|app| folder.join(app)))
            .collect()
    } else {
        Vec::new()
    };
    found.into_iter().find(|path| path.is_file())
}

/// What to install when no browser is found, said in the tab.
pub const INSTALL: &str = if cfg!(windows) {
    "Install Microsoft Edge to browse here."
} else {
    "Install Google Chrome or Microsoft Edge to browse here."
};

/// What the browser is started with: no window, its own profile, no first-run pages, no sign-in to
/// the browser itself and no sync, and the debugging pipe.
pub fn flags(program: &Path, profile: &Path) -> Vec<String> {
    let mut flags = vec![
        "--headless".to_string(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-sync".into(),
        // Edge's own sign-in to the browser, which a new profile otherwise does with the PC's account.
        "--disable-features=msImplicitSignin,msEnableWebToBrowserSignIn,msSeamlessWebToBrowserSignIn".into(),
        // The pipe alone makes `navigator.webdriver` true, and a sign-in page may then refuse the
        // person: it is their own browser, used by them.
        "--disable-blink-features=AutomationControlled".into(),
        // Without a window the browser takes the PC's dark theme for the page's own scroll bar when
        // the page names no colour scheme, and draws it dark on a light page. It is light then, as
        // a page is (1 is light); what pages are told to prefer is unchanged.
        "--blink-settings=preferredRootScrollbarColorScheme=1".into(),
        format!("--user-data-dir={}", profile.display()),
        "--remote-debugging-pipe".into(),
        "--no-startup-window".into(),
    ];
    // A test, or a builder's run on a PC with the owner's account, starts the browser private. Pages
    // opened over the pipe still use the profile: its cookies are kept, as the person's are.
    if cfg!(debug_assertions) && std::env::var_os("GATHR_BROWSER_PRIVATE").is_some() {
        let edge = program.to_string_lossy().to_lowercase().contains("edge");
        flags.push(if edge { "--inprivate" } else { "--incognito" }.into());
    }
    flags
}

/// Starts the browser, its pipe on the child's stdin and stdout, in the job (or process group) that
/// ends it with the service. Chromium reads the pipe from handles named on its command line on
/// Windows, which only an inheritable handle can be: `gathrd` relays its own stdin and stdout to it.
/// On Unix it reads file descriptors 3 and 4, which the shell sets up.
fn launch(
    program: &Path,
    profile: &Path,
) -> std::io::Result<Box<dyn process_wrap::tokio::ChildWrapper>> {
    let flags = flags(program, profile);
    let mut command = if cfg!(windows) {
        let mut command = tokio::process::Command::new(std::env::current_exe()?);
        // Edge given a compatibility layer starts itself again without it, as a new process, and the pipe ends.
        command.arg(RELAY).arg(program).env_remove("__COMPAT_LAYER");
        command
    } else {
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(r#"exec "$0" "$@" 3<&0 4>&1 </dev/null >/dev/null"#)
            .arg(program);
        command
    };
    command
        .args(flags)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut wrapped = crate::platform::ending_together(command);
    wrapped.wrap(process_wrap::tokio::KillOnDrop);
    wrapped.spawn()
}

/// The process started itself, not its job: the browser's helpers can outlive it, and holding its
/// pipe, its crash handler does.
fn own(
    mut child: &mut dyn process_wrap::tokio::ChildWrapper,
) -> &mut dyn process_wrap::tokio::ChildWrapper {
    while !(child as &dyn std::any::Any).is::<tokio::process::Child>() {
        child = child.inner_mut();
    }
    child
}

/// `gathrd --browser-relay <browser> <flags>`: runs the browser with this process's stdin and stdout
/// as its debugging pipe, until it ends. None when not asked.
pub fn relay() -> Option<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != RELAY {
        return None;
    }
    let program = args.next()?;
    let mut command = std::process::Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let input = std::io::stdin().as_raw_handle() as usize;
        let output = std::io::stdout().as_raw_handle() as usize;
        command.arg(format!("--remote-debugging-io-pipes={input},{output}"));
    }
    let ended = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Some(match ended {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

/// The browser running now.
struct Live {
    cdp: Arc<cdp::Cdp>,
    heard: mpsc::UnboundedReceiver<cdp::Heard>,
    /// Dropped or sent, it stops the browser.
    _kill: oneshot::Sender<()>,
    /// Ends when the browser's own process has ended.
    gone: oneshot::Receiver<()>,
    started: Instant,
}

/// Starts and restarts the browser; `pages` does the rest.
impl pages::Actor {
    pub(crate) async fn run(mut self, mut cmds: mpsc::UnboundedReceiver<Cmd>) {
        enum Next {
            Cmd(Option<Cmd>),
            Heard(Option<cdp::Heard>),
            Restart,
        }
        loop {
            let next = {
                let heard = async {
                    match self.live.as_mut() {
                        Some(live) => live.heard.recv().await,
                        None => std::future::pending().await,
                    }
                };
                let restart = async {
                    match self.restart {
                        Some(at) => tokio::time::sleep_until(at.into()).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    cmd = cmds.recv() => Next::Cmd(cmd),
                    heard = heard => Next::Heard(heard),
                    () = restart => Next::Restart,
                }
            };
            match next {
                Next::Cmd(None) => return,
                Next::Cmd(Some(Cmd::Stop(done))) => {
                    self.close().await;
                    let _ = done.send(());
                    return;
                }
                Next::Cmd(Some(Cmd::Ended(start, how))) => self.ended(start, how),
                Next::Cmd(Some(cmd)) => self.on_cmd(cmd).await,
                Next::Heard(Some(heard)) => self.on_heard(heard).await,
                Next::Heard(None) => self.ended(self.starts, "its pipe closed".into()),
                Next::Restart => {
                    self.restart = None;
                    self.start().await;
                }
            }
        }
    }

    /// Starts the browser if it isn't running or waiting to start again.
    pub(crate) async fn start(&mut self) {
        if self.live.is_some() || self.restart.is_some() {
            return;
        }
        let Some(program) = find() else {
            self.missing = true;
            return self.publish();
        };
        let _ = std::fs::create_dir_all(&self.profile);
        self.starts += 1;
        let mut child = match launch(&program, &self.profile) {
            Ok(child) => child,
            Err(e) => return self.failed(&e.to_string()),
        };
        let pid = child.id().unwrap_or_default();
        let (Some(stdin), Some(stdout)) = (child.stdin().take(), child.stdout().take()) else {
            return self.failed("its pipe didn't open");
        };
        let (cdp, heard) = cdp::Cdp::start(stdin, stdout);
        let (kill, killed) = oneshot::channel::<()>();
        let (went, gone) = oneshot::channel();
        let (said, start) = (self.said.clone(), self.starts);
        tokio::spawn(async move {
            let mut killed = killed;
            let ended = tokio::select! {
                ended = own(child.as_mut()).wait() => Some(ended),
                _ = &mut killed => None,
            };
            let _ = went.send(());
            let how = match ended {
                Some(Ok(status)) => status.to_string(),
                Some(Err(e)) => e.to_string(),
                None => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    "stopped".into()
                }
            };
            let _ = said.send(Cmd::Ended(start, how));
        });
        self.live = Some(Live {
            cdp: cdp.clone(),
            heard,
            _kill: kill,
            gone,
            started: Instant::now(),
        });
        match self.set_up(&cdp).await {
            Ok(()) => {
                self.writer
                    .record(Event::point("gathr.browser.start").ok().attr("pid", pid));
                self.publish();
            }
            Err(why) => {
                self.live = None;
                self.failed(&why);
                // A browser left running on this profile, by a service that ended without it or by hand,
                // takes each new one's start and closes it: it is ended, and the start tried again now.
                let profile = self.profile.clone();
                let ended = tokio::task::spawn_blocking(move || end_strays(&profile, pid))
                    .await
                    .unwrap_or_default();
                if ended > 0 {
                    self.writer.record(
                        Event::point("gathr.browser.stray")
                            .ok()
                            .attr("ended", ended as i64),
                    );
                    self.restart = Some(Instant::now());
                }
            }
        }
        // An address opened while the browser wasn't running opens now.
        if self.live.is_some() && self.opening.is_some() {
            self.open_waiting();
        }
    }

    /// A start that failed: recorded, and tried again after a wait.
    fn failed(&mut self, why: &str) {
        let event = Event::point("gathr.browser.start")
            .error("start")
            .attr("why", why);
        self.writer.record(event);
        self.again();
    }

    fn again(&mut self) {
        self.restart = Some(Instant::now() + self.wait);
        self.wait = (self.wait * 2).min(LONGEST_WAIT);
        self.publish();
    }

    /// The browser of start `start` ended. Unless the service is stopping it, it died: its pages
    /// are gone, and it starts again.
    fn ended(&mut self, start: u64, how: String) {
        if start != self.starts || self.live.is_none() {
            return;
        }
        let ran = self
            .live
            .take()
            .map(|l| l.started.elapsed())
            .unwrap_or_default();
        self.writer.record(
            Event::point("gathr.browser.crash")
                .error("exited")
                .attr("how", how)
                .attr("ran_ms", ran.as_millis() as i64),
        );
        self.forget();
        if ran > LONGEST_WAIT {
            self.wait = Duration::from_secs(1);
        }
        self.again();
    }

    /// At the service's stop: the browser closes itself, so its cookies are written, else is stopped.
    async fn close(&mut self) {
        let Some(mut live) = self.live.take() else {
            return;
        };
        let cdp = live.cdp.clone();
        let closing = async {
            let _ = cdp.call("Browser.close", json!({}), None).await;
            let _ = (&mut live.gone).await;
        };
        let closed = tokio::time::timeout(CLOSING, closing).await.is_ok();
        // Dropping it stops what is left.
        drop(live);
        let event = Event::point("gathr.browser.stop")
            .ok()
            .attr("closed", closed);
        self.writer.record_now(event).await;
    }
}

/// Each window's socket and what it last said of its tab: its size and whether it shows the page.
pub(crate) type Watchers = HashMap<u64, pages::Watcher>;

/// Ends every browser left running on `profile` other than `ours`, the one just started: the main
/// process of each, which takes its helpers with it. How many were ended.
fn end_strays(profile: &Path, ours: u32) -> usize {
    let profile = profile.to_string_lossy().to_lowercase();
    let strays: Vec<u32> = processes()
        .into_iter()
        .filter(|(pid, line)| {
            let line = line.to_lowercase();
            *pid != ours
                && line.contains(&format!("--user-data-dir={profile}"))
                && !line.contains("--type=")
                && !line.contains(RELAY)
        })
        .map(|(pid, _)| pid)
        .collect();
    for pid in &strays {
        let mut kill = if cfg!(windows) {
            let mut kill = std::process::Command::new("taskkill");
            kill.args(["/F", "/T", "/PID", &pid.to_string()]);
            kill
        } else {
            let mut kill = std::process::Command::new("kill");
            kill.args(["-9", &pid.to_string()]);
            kill
        };
        crate::platform::no_window(&mut kill);
        let _ = kill.stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    strays.len()
}

/// Every running process's id and command line.
fn processes() -> Vec<(u32, String)> {
    let mut list = if cfg!(windows) {
        let mut list = std::process::Command::new("powershell");
        let script = "Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId) $($_.CommandLine)\" }";
        list.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        list
    } else {
        let mut list = std::process::Command::new("ps");
        list.args(["-axww", "-o", "pid=,command="]);
        list
    };
    crate::platform::no_window(&mut list);
    let Ok(listed) = list.stdin(Stdio::null()).stderr(Stdio::null()).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(|line| {
            let (pid, rest) = line.trim().split_once(' ')?;
            Some((pid.parse().ok()?, rest.to_string()))
        })
        .collect()
}
