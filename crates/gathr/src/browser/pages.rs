//! The browser's pages as the windows see them: its tabs, the one shown, its live picture at the
//! tab's size, its pointer and its own dialogs, and the person's mouse, keys and answers. Nothing of a
//! page (its address, title, words or a dialog's) is recorded or stored: it goes only to the windows.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

use super::cdp::{Cdp, Heard};
use super::{Cmd, Live, Watchers};
use crate::writer::Writer;

/// The shortest time between two frames of the live picture: at most about 30 a second.
const FRAME: Duration = Duration::from_millis(33);
const DOWNLOADS: &str = "Downloads aren't built yet";
const UPLOADS: &str = "Uploads aren't built yet";
const BLANK: &str = "about:blank";
const ARROW: &str = "default";
/// What pointer a page shows at a place: what it computes there, looking into same-origin frames and
/// open shadow roots, which a look from the document stops at.
const CURSOR: &str = include_str!("cursor.js");

/// What goes to the windows: the tabs' state and other JSON, or a frame of the shown page (a JPEG).
#[derive(Clone)]
pub enum Out {
    Text(Arc<str>),
    Frame(Bytes),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Size {
    width: u32,
    height: u32,
    ratio: f64,
}

#[derive(Default)]
pub(crate) struct Watcher {
    size: Option<Size>,
    visible: bool,
}

struct Page {
    id: String,
    session: String,
    title: String,
    url: String,
    loading: bool,
    back: bool,
    forward: bool,
    size: Option<Size>,
    /// Its own dialog, open and waiting for the person: `{"kind", "message", "value"}`. The page is
    /// stopped meanwhile and answers nothing.
    dialog: Option<Value>,
    /// What pointer it shows is being asked.
    asking: bool,
}

impl Page {
    /// Asking whether to leave: a move elsewhere waits behind the question.
    fn leaving(&self) -> bool {
        self.dialog
            .as_ref()
            .is_some_and(|d| d["kind"] == "beforeunload")
    }
}

pub(crate) struct Actor {
    pub(super) profile: PathBuf,
    pub(super) writer: Writer,
    out: broadcast::Sender<Out>,
    state: Arc<Mutex<Out>>,
    pub(super) said: mpsc::UnboundedSender<Cmd>,
    pub(super) live: Option<Live>,
    /// How many times the browser was started: an old start's end is told apart.
    pub(super) starts: u64,
    pub(super) restart: Option<Instant>,
    pub(super) wait: Duration,
    pub(super) missing: bool,
    pages: Vec<Page>,
    shown: Option<String>,
    watchers: Watchers,
    /// The window that acted last: its tab's size is the page's.
    sizer: Option<u64>,
    /// The page whose picture is sent, by its session, at what size.
    casting: Option<(String, Size)>,
    /// The address the next page to open goes to, shown: a new page's, or one the person opened with
    /// none open.
    opening: Option<String>,
    line: Option<&'static str>,
    /// The pointer the windows were last told, a CSS cursor as the page said it.
    cursor: String,
    /// The newest place the mouse moved to over a page, by its id, not yet asked about.
    pointing: Option<(String, f64, f64)>,
    agent: String,
    acked: Instant,
}

impl Actor {
    pub(super) fn new(
        profile: PathBuf,
        writer: Writer,
        out: broadcast::Sender<Out>,
        state: Arc<Mutex<Out>>,
        said: mpsc::UnboundedSender<Cmd>,
    ) -> Actor {
        let actor = Actor {
            profile,
            writer,
            out,
            state,
            said,
            live: None,
            starts: 0,
            restart: None,
            wait: Duration::from_secs(1),
            missing: false,
            pages: Vec::new(),
            shown: None,
            watchers: Watchers::new(),
            sizer: None,
            casting: None,
            opening: None,
            line: None,
            cursor: ARROW.into(),
            pointing: None,
            agent: String::new(),
            acked: Instant::now(),
        };
        actor.publish();
        actor
    }

    fn cdp(&self) -> Option<Arc<Cdp>> {
        self.live.as_ref().map(|l| l.cdp.clone())
    }

    fn page(&self, id: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == id)
    }

    fn shown_page(&self) -> Option<&Page> {
        self.page(self.shown.as_deref()?)
    }

    /// Once started: sites see a plain browser, downloads are refused, so are the camera, the
    /// microphone, the location and notifications; every page, and each one opened later, is told.
    pub(super) async fn set_up(&mut self, cdp: &Cdp) -> Result<(), String> {
        let version = cdp.call("Browser.getVersion", json!({}), None).await?;
        self.agent = version["userAgent"]
            .as_str()
            .unwrap_or_default()
            .replace("Headless", "");
        let downloads = json!({ "behavior": "deny", "eventsEnabled": true });
        cdp.call("Browser.setDownloadBehavior", downloads, None)
            .await?;
        for name in ["geolocation", "notifications", "camera", "microphone"] {
            let refused = json!({ "permission": { "name": name }, "setting": "denied" });
            cdp.call("Browser.setPermission", refused, None).await?;
        }
        cdp.call(
            "Target.setDiscoverTargets",
            json!({ "discover": true }),
            None,
        )
        .await?;
        let attach = json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true });
        cdp.call("Target.setAutoAttach", attach, None).await?;
        Ok(())
    }

    /// The browser is gone: so are its pages.
    pub(super) fn forget(&mut self) {
        self.pages.clear();
        self.shown = None;
        self.casting = None;
        self.opening = None;
        self.point(ARROW);
    }

    pub(super) async fn on_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Said(client, message) => self.on_said(client, message).await,
            Cmd::Left(client) => {
                self.watchers.remove(&client);
                if self.sizer == Some(client) {
                    self.sizer = None;
                }
                self.fit();
            }
            Cmd::Looked(looked) => {
                let Some(page) = self.pages.iter_mut().find(|p| p.id == looked.id) else {
                    return;
                };
                let was = (&page.title, &page.url, page.back, page.forward);
                if was != (&looked.title, &looked.url, looked.back, looked.forward) {
                    (page.title, page.url) = (looked.title, looked.url);
                    (page.back, page.forward) = (looked.back, looked.forward);
                    self.publish();
                }
            }
            Cmd::Moved(id, x, y) => {
                self.pointing = Some((id, x, y));
                self.ask();
            }
            Cmd::Pointed(id, cursor) => {
                if let Some(page) = self.pages.iter_mut().find(|p| p.id == id) {
                    page.asking = false;
                }
                let shown = self.shown_page();
                if shown.is_some_and(|p| p.id == id && p.dialog.is_none())
                    && let Some(cursor) = cursor
                {
                    self.point(&cursor);
                }
                self.ask();
            }
            Cmd::Stop(_) | Cmd::Ended(..) => {}
        }
    }

    async fn on_said(&mut self, client: u64, message: Value) {
        let kind = message["type"].as_str().unwrap_or_default();
        if kind == "view" {
            let watcher = self.watchers.entry(client).or_default();
            watcher.visible = message["visible"].as_bool().unwrap_or(false);
            let size = || {
                let ratio = message["ratio"]
                    .as_f64()
                    .filter(|r| (0.5..=4.0).contains(r))?;
                let side = |key| message[key].as_u64().filter(|n| (50..=10_000).contains(n));
                Some(Size {
                    width: side("width")? as u32,
                    height: side("height")? as u32,
                    ratio,
                })
            };
            if let Some(size) = size() {
                watcher.size = Some(size);
            }
            if watcher.visible {
                self.sizer = Some(client);
                self.start().await;
            }
            return self.fit();
        }
        // The window the person uses sets the page's size.
        self.sizer = Some(client);
        self.fit();
        if kind == "open" || kind == "new" {
            let typed = (kind == "open").then(|| message["address"].as_str().unwrap_or_default());
            return self.open(typed).await;
        }
        if kind == "answer" {
            return self.answer(&message);
        }
        if kind == "show" || kind == "close" {
            let Some(id) = message["page"]
                .as_str()
                .filter(|id| self.page(id).is_some())
            else {
                return;
            };
            if kind == "close" {
                if let Some(cdp) = self.cdp() {
                    cdp.send("Target.closeTarget", json!({ "targetId": id }), None);
                }
                return;
            }
            self.shown = Some(id.to_string());
            self.line = None;
            self.point(ARROW);
            self.fit();
            return self.publish();
        }
        let (Some(cdp), Some(page)) = (self.cdp(), self.shown_page()) else {
            return;
        };
        let (id, session, leaving) = (page.id.clone(), page.session.clone(), page.leaving());
        match kind {
            "back" | "forward" | "reload" => {
                let step = if kind == "back" { -1 } else { 1 };
                let reload = kind == "reload";
                tokio::spawn(async move {
                    stay(&cdp, &session, leaving).await;
                    if reload {
                        return cdp.call("Page.reload", json!({}), Some(&session)).await;
                    }
                    let history = cdp
                        .call("Page.getNavigationHistory", json!({}), Some(&session))
                        .await?;
                    let at = history["currentIndex"].as_i64().unwrap_or_default() + step;
                    let entry = &history["entries"][at.max(0) as usize]["id"];
                    if at >= 0 && entry.is_i64() {
                        let to = json!({ "entryId": entry });
                        cdp.call("Page.navigateToHistoryEntry", to, Some(&session))
                            .await?;
                    }
                    Ok(Value::Null)
                });
            }
            "stop" => cdp.send("Page.stopLoading", json!({}), Some(&session)),
            // A page with a dialog open is stopped: what is sent would all run once it is answered.
            "mouse" | "wheel" | "key" | "paste" if page.dialog.is_none() => {
                self.input(&cdp, (id, session), kind, &message)
            }
            _ => {}
        }
    }

    /// Goes to what was typed in the address, in the page shown; or opens a new page (`None`), or one
    /// for the address when none is shown, and shows it.
    async fn open(&mut self, typed: Option<&str>) {
        let url = typed.map_or_else(|| BLANK.to_string(), address);
        if url.is_empty() {
            return;
        }
        self.start().await;
        let Some(cdp) = self.cdp() else {
            return;
        };
        self.line = None;
        match self.shown_page().filter(|_| typed.is_some()) {
            Some(page) => {
                let (session, leaving) = (page.session.clone(), page.leaving());
                tokio::spawn(async move {
                    stay(&cdp, &session, leaving).await;
                    let _ = cdp
                        .call("Page.navigate", json!({ "url": url }), Some(&session))
                        .await;
                });
            }
            // Blank first, so it is sized before it loads.
            None => {
                self.opening = Some(url);
                cdp.send("Target.createTarget", json!({ "url": BLANK }), None);
            }
        }
        self.publish();
    }

    /// The person's answer to a page's dialog: `accept`, and the text typed for a prompt.
    fn answer(&self, m: &Value) {
        let page = m["page"].as_str().and_then(|id| self.page(id));
        let (Some(cdp), Some(page)) = (self.cdp(), page.filter(|p| p.dialog.is_some())) else {
            return;
        };
        let mut answer = json!({ "accept": m["accept"].as_bool().unwrap_or(false) });
        if let Some(text) = m["text"].as_str() {
            answer["promptText"] = json!(text);
        }
        cdp.send("Page.handleJavaScriptDialog", answer, Some(&page.session));
    }

    /// Tells the windows the pointer to show over the page, when it changed.
    fn point(&mut self, cursor: &str) {
        if self.cursor != cursor {
            self.cursor = cursor.to_string();
            let said = json!({ "type": "cursor", "value": cursor }).to_string();
            let _ = self.out.send(Out::Text(said.into()));
        }
    }

    /// Asks the shown page what pointer it shows where the mouse last moved: one question to a page at
    /// a time, the newest place winning, and none while it has a dialog open, as it answers nothing then.
    fn ask(&mut self) {
        let Some(cdp) = self.cdp() else {
            return;
        };
        let shown = self.shown.clone();
        let Some(page) = self
            .pages
            .iter_mut()
            .find(|p| Some(&p.id) == shown.as_ref())
        else {
            return;
        };
        if page.asking || page.dialog.is_some() {
            return;
        }
        let Some((id, x, y)) = self.pointing.take().filter(|(id, ..)| *id == page.id) else {
            return;
        };
        page.asking = true;
        let (session, said) = (page.session.clone(), self.said.clone());
        let expression = format!("{}({x}, {y})", CURSOR.trim_end());
        let read = json!({ "expression": expression, "returnByValue": true });
        tokio::spawn(async move {
            let read = cdp.call("Runtime.evaluate", read, Some(&session)).await;
            let cursor = read
                .ok()
                .and_then(|r| r["result"]["value"].as_str().map(String::from));
            let _ = said.send(Cmd::Pointed(id, cursor));
        });
    }

    /// The person's mouse, wheel, keys and paste, to the page shown. A release that may have changed
    /// what is selected reads it, for the window's copy, and looks at the page again; a move is
    /// followed by asking the page what pointer it shows there.
    fn input(&self, cdp: &Arc<Cdp>, (id, session): (String, String), kind: &str, m: &Value) {
        let num = |key: &str| m[key].as_f64().unwrap_or_default();
        let modifiers = m["modifiers"].as_u64().unwrap_or_default() & 15;
        let (method, params) = match kind {
            "mouse" => {
                let event = m["event"].as_str().unwrap_or_default();
                let button = m["button"].as_str().unwrap_or("none");
                let known =
                    ["none", "left", "middle", "right", "back", "forward"].contains(&button);
                if !["mouseMoved", "mousePressed", "mouseReleased"].contains(&event) || !known {
                    return;
                }
                let params = json!({
                    "type": event, "x": num("x"), "y": num("y"), "button": button, "modifiers": modifiers,
                    "buttons": m["buttons"].as_u64().unwrap_or_default(),
                    "clickCount": m["clickCount"].as_u64().unwrap_or_default(),
                });
                ("Input.dispatchMouseEvent", params)
            }
            "wheel" => {
                let params = json!({
                    "type": "mouseWheel", "x": num("x"), "y": num("y"), "modifiers": modifiers,
                    "deltaX": num("dx"), "deltaY": num("dy"),
                });
                ("Input.dispatchMouseEvent", params)
            }
            "key" => {
                let text = m["text"].as_str().unwrap_or_default();
                let event = match m["event"].as_str() {
                    Some("keyUp") => "keyUp",
                    Some("keyDown") if text.is_empty() => "rawKeyDown",
                    Some("keyDown") => "keyDown",
                    _ => return,
                };
                let code = m["keyCode"].as_u64().unwrap_or_default();
                let params = json!({
                    "type": event, "modifiers": modifiers, "text": text, "unmodifiedText": text,
                    "key": m["key"].as_str().unwrap_or_default(), "code": m["code"].as_str().unwrap_or_default(),
                    "windowsVirtualKeyCode": code, "nativeVirtualKeyCode": code,
                    "location": m["location"].as_u64().unwrap_or_default(),
                    "autoRepeat": m["repeat"].as_bool().unwrap_or(false),
                });
                ("Input.dispatchKeyEvent", params)
            }
            _ => {
                let text = m["text"].as_str().unwrap_or_default();
                ("Input.insertText", json!({ "text": text }))
            }
        };
        let released = params["type"] == "mouseReleased" || params["type"] == "keyUp";
        let selects = kind == "mouse" && released || kind == "key" && released && modifiers != 0;
        let changes = released || kind == "paste";
        let moved = (params["type"] == "mouseMoved").then(|| (num("x"), num("y")));
        let (cdp, out, said) = (cdp.clone(), self.out.clone(), self.said.clone());
        tokio::spawn(async move {
            let _ = cdp.call(method, params, Some(&session)).await;
            if let Some((x, y)) = moved {
                let _ = said.send(Cmd::Moved(id.clone(), x, y));
            }
            if changes {
                tokio::spawn(look_at(cdp.clone(), said, id, session.clone()));
            }
            if selects {
                let read = json!({ "expression": SELECTION, "returnByValue": true });
                if let Ok(read) = cdp.call("Runtime.evaluate", read, Some(&session)).await {
                    let text = read["result"]["value"].as_str().unwrap_or_default();
                    let said = json!({ "type": "selection", "text": text }).to_string();
                    let _ = out.send(Out::Text(said.into()));
                }
            }
        });
    }

    pub(super) async fn on_heard(&mut self, heard: Heard) {
        let Heard {
            method,
            params,
            session,
        } = heard;
        let Some(cdp) = self.cdp() else {
            return;
        };
        match method.as_str() {
            "Target.attachedToTarget" => self.attached(&cdp, &params),
            "Target.targetInfoChanged" => {
                let id = params["targetInfo"]["targetId"]
                    .as_str()
                    .unwrap_or_default();
                if let Some(page) = self.page(id) {
                    look(&cdp, &self.said, page);
                }
            }
            "Target.targetDestroyed" | "Target.detachedFromTarget" => {
                let id = params["targetId"].as_str();
                let session = params["sessionId"].as_str();
                let at = self
                    .pages
                    .iter()
                    .position(|p| Some(p.id.as_str()) == id || Some(p.session.as_str()) == session);
                if let Some(at) = at {
                    let gone = self.pages.remove(at);
                    if self.shown.as_deref() == Some(gone.id.as_str()) {
                        let next = self.pages.get(at).or(self.pages.last());
                        self.shown = next.map(|p| p.id.clone());
                        self.point(ARROW);
                    }
                    if self
                        .casting
                        .as_ref()
                        .is_some_and(|(s, _)| *s == gone.session)
                    {
                        self.casting = None;
                    }
                    self.fit();
                }
            }
            "Browser.downloadWillBegin" => {
                self.line = Some(DOWNLOADS);
                self.publish();
            }
            _ => {
                if let Some(session) = session {
                    self.on_page(&cdp, &session, &method, &params);
                }
            }
        }
    }

    fn on_page(&mut self, cdp: &Arc<Cdp>, session: &str, method: &str, params: &Value) {
        let Some(page) = self.pages.iter_mut().find(|p| p.session == session) else {
            return;
        };
        match method {
            "Page.frameStartedLoading" | "Page.frameStoppedLoading"
                if params["frameId"] == page.id.as_str() =>
            {
                page.loading = method == "Page.frameStartedLoading";
                look(cdp, &self.said, page);
                // What the line said, and the pointer, were of the page before.
                if page.loading && self.shown.as_deref() == Some(page.id.as_str()) {
                    self.line = None;
                    self.point(ARROW);
                }
                self.publish();
            }
            "Page.navigatedWithinDocument" => look(cdp, &self.said, page),
            "Page.screencastFrame" => {
                let casting = self.casting.as_ref().is_some_and(|(s, _)| s == session);
                if let (true, Some(frame)) =
                    (casting, decode(params["data"].as_str().unwrap_or_default()))
                {
                    let _ = self.out.send(Out::Frame(frame.into()));
                }
                // The next frame comes once this one is answered: at most about 30 a second.
                let now = Instant::now();
                let at = (self.acked + FRAME).max(now);
                self.acked = at;
                let ack = json!({ "sessionId": params["sessionId"] });
                let (cdp, session) = (cdp.clone(), session.to_string());
                tokio::spawn(async move {
                    tokio::time::sleep_until(at.into()).await;
                    let _ = cdp
                        .call("Page.screencastFrameAck", ack, Some(&session))
                        .await;
                });
            }
            // A page's own alert, confirm, prompt or question on leaving waits for the person's answer.
            "Page.javascriptDialogOpening" => {
                let (kind, message) = (&params["type"], &params["message"]);
                page.dialog = Some(json!({
                    "kind": kind, "message": message, "value": params["defaultPrompt"],
                }));
                self.publish();
            }
            "Page.javascriptDialogClosed" => {
                page.dialog = None;
                self.publish();
            }
            "Page.fileChooserOpened" => {
                self.line = Some(UPLOADS);
                self.publish();
            }
            _ => {}
        }
    }

    /// A new page, or another kind of target, waiting to run: a page is set up first, at the size
    /// of the tab, so it never shows at another size.
    fn attached(&mut self, cdp: &Arc<Cdp>, params: &Value) {
        let session = params["sessionId"].as_str().unwrap_or_default().to_string();
        let info = &params["targetInfo"];
        if info["type"] != "page" {
            cdp.send("Runtime.runIfWaitingForDebugger", json!({}), Some(&session));
            return;
        }
        let size = self.size();
        let page = Page {
            id: info["targetId"].as_str().unwrap_or_default().to_string(),
            session: session.clone(),
            title: info["title"].as_str().unwrap_or_default().to_string(),
            url: info["url"].as_str().unwrap_or_default().to_string(),
            loading: false,
            back: false,
            forward: false,
            size,
            dialog: None,
            asking: false,
        };
        let opening = self.opening.take();
        // A page the shown one opened, by a link to a new window or `window.open`, is shown, as an
        // everyday browser brings it to the front. The browser names its opener even for `noopener`.
        let opened = self.shown.is_some() && info["openerId"].as_str() == self.shown.as_deref();
        if opening.is_some() || self.shown.is_none() || opened {
            self.shown = Some(page.id.clone());
            self.point(ARROW);
        }
        self.pages.push(page);
        let (cdp, agent) = (cdp.clone(), self.agent.clone());
        // Sent at once, in this order, never waiting on one: a page opened by another one answers
        // nothing until it runs.
        tokio::spawn(async move {
            let on = Some(session.as_str());
            let sized = match size {
                Some(size) => ("Emulation.setDeviceMetricsOverride", metrics(size)),
                None => ("Emulation.clearDeviceMetricsOverride", json!({})),
            };
            let _ = tokio::join!(
                cdp.call("Page.enable", json!({}), on),
                cdp.call(
                    "Page.setInterceptFileChooserDialog",
                    json!({ "enabled": true }),
                    on
                ),
                cdp.call(
                    "Emulation.setFocusEmulationEnabled",
                    json!({ "enabled": true }),
                    on
                ),
                cdp.call(
                    "Emulation.setUserAgentOverride",
                    json!({ "userAgent": agent }),
                    on
                ),
                cdp.call(sized.0, sized.1, on),
                cdp.call("Runtime.runIfWaitingForDebugger", json!({}), on),
            );
            if let Some(url) = opening.filter(|url| url != BLANK) {
                let _ = cdp.call("Page.navigate", json!({ "url": url }), on).await;
            }
        });
        self.fit();
    }

    /// The size of the tab of the window that acted last, else of any window that told its size.
    fn size(&self) -> Option<Size> {
        let sizer = self.sizer.and_then(|s| self.watchers.get(&s)?.size);
        sizer.or_else(|| self.watchers.values().find_map(|w| w.size))
    }

    /// Lays the shown page out at the tab's size, and sends its picture while a window shows it.
    pub(super) fn fit(&mut self) {
        let size = self.size();
        let cdp = self.cdp();
        let shown = self.shown.clone();
        let page = self
            .pages
            .iter_mut()
            .find(|p| Some(&p.id) == shown.as_ref());
        if let (Some(cdp), Some(page), Some(size)) = (&cdp, page, size)
            && page.size != Some(size)
        {
            page.size = Some(size);
            cdp.send(
                "Emulation.setDeviceMetricsOverride",
                metrics(size),
                Some(&page.session),
            );
        }
        let seen = self.watchers.values().any(|w| w.visible);
        let want = match (seen, self.shown_page(), size) {
            (true, Some(page), Some(size)) => Some((page.session.clone(), size)),
            _ => None,
        };
        if want != self.casting
            && let Some(cdp) = cdp
        {
            if let Some((session, _)) = &self.casting {
                cdp.send("Page.stopScreencast", json!({}), Some(session));
            }
            if let Some((session, size)) = &want {
                let device = |side: u32| (side as f64 * size.ratio).ceil() as u32;
                let cast = json!({
                    "format": "jpeg", "quality": 90, "everyNthFrame": 1,
                    "maxWidth": device(size.width), "maxHeight": device(size.height),
                });
                cdp.send("Page.startScreencast", cast, Some(session));
            }
            self.casting = want;
        }
        self.publish();
    }

    /// The tabs' state, to every window and kept for the next to connect.
    pub(super) fn publish(&self) {
        let pages: Vec<Value> = self
            .pages
            .iter()
            .map(|p| {
                json!({
                    "id": p.id, "title": p.title, "url": p.url, "loading": p.loading,
                    "back": p.back, "forward": p.forward, "dialog": p.dialog,
                })
            })
            .collect();
        let state = json!({
            "type": "state",
            "running": self.live.is_some(),
            "missing": self.missing.then_some(super::INSTALL),
            "pages": pages,
            "shown": self.shown,
            "line": self.line,
        });
        let state = Out::Text(state.to_string().into());
        if let Ok(mut kept) = self.state.lock() {
            *kept = state.clone();
        }
        let _ = self.out.send(state);
    }
}

/// A page asking whether to leave holds back a move elsewhere, and would ask again once answered: it
/// stays first, and then asks about the new move.
async fn stay(cdp: &Cdp, session: &str, leaving: bool) {
    if leaving {
        let stay = json!({ "accept": false });
        let _ = cdp
            .call("Page.handleJavaScriptDialog", stay, Some(session))
            .await;
    }
}

/// Reads a page's title, address and history again. The browser tells of a new address, but not
/// always of a new title: each load, move within the page and use of it is followed by a look.
fn look(cdp: &Arc<Cdp>, said: &mpsc::UnboundedSender<Cmd>, page: &Page) {
    let (cdp, said, id, session) = (
        cdp.clone(),
        said.clone(),
        page.id.clone(),
        page.session.clone(),
    );
    tokio::spawn(look_at(cdp, said, id, session));
}

async fn look_at(cdp: Arc<Cdp>, said: mpsc::UnboundedSender<Cmd>, id: String, session: String) {
    let info = cdp
        .call("Target.getTargetInfo", json!({ "targetId": id }), None)
        .await;
    let history = cdp
        .call("Page.getNavigationHistory", json!({}), Some(&session))
        .await;
    let (Ok(info), Ok(history)) = (info, history) else {
        return;
    };
    let info = &info["targetInfo"];
    let text = |key: &str| info[key].as_str().unwrap_or_default().to_string();
    let at = history["currentIndex"].as_u64().unwrap_or_default() as usize;
    let count = history["entries"].as_array().map_or(0, Vec::len);
    let looked = Looked {
        id,
        title: text("title"),
        url: text("url"),
        back: at > 0,
        forward: at + 1 < count,
    };
    let _ = said.send(Cmd::Looked(looked));
}

/// A page as `look` found it.
pub(crate) struct Looked {
    id: String,
    title: String,
    url: String,
    back: bool,
    forward: bool,
}

fn metrics(size: Size) -> Value {
    json!({ "width": size.width, "height": size.height, "deviceScaleFactor": size.ratio, "mobile": false })
}

/// What is selected in the page: in a field being typed in, else in the page.
const SELECTION: &str = "(() => { const a = document.activeElement; \
    if (a && typeof a.selectionStart === 'number' && typeof a.value === 'string') \
    return a.value.slice(a.selectionStart, a.selectionEnd); return String(getSelection()); })()";

/// What the person typed as an address: a web address as typed, a site's name made one, and any other
/// words searched for with Bing.
fn address(typed: &str) -> String {
    let typed = typed.trim();
    let lower = typed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || typed.is_empty() {
        return typed.to_string();
    }
    let host = typed.split(['/', '?', '#']).next().unwrap_or_default();
    let local = ["localhost", "127.0.0.1", "[::1]"]
        .iter()
        .any(|l| host.starts_with(l));
    let named = host.contains('.') && !host.starts_with('.') && !host.ends_with('.');
    if !typed.contains(char::is_whitespace) && (local || named) {
        let scheme = if local { "http" } else { "https" };
        return format!("{scheme}://{typed}");
    }
    let mut q = String::new();
    for byte in typed.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                q.push(byte as char)
            }
            b' ' => q.push('+'),
            _ => q.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("https://www.bing.com/search?q={q}")
}

/// Standard base64, as the browser sends a frame.
fn decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let text = text.trim_end_matches('=').as_bytes();
    let mut bytes = Vec::with_capacity(text.len() * 3 / 4);
    for chunk in text.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= u32::from(value(c)?) << (18 - 6 * i);
        }
        let got = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        bytes.extend_from_slice(&got[..chunk.len().saturating_sub(1)]);
    }
    Some(bytes)
}
