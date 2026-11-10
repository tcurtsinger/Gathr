//! Codex authentication uses app-server notifications, never a pasted code (0033).
use super::*;
use gathr::accounts::Account;
use gathr::codex::{client::Client, command as codex};

pub async fn start(s: Arc<State>, t: Trace, account: Account, device: bool) -> Answer {
    let sign_in = s.sign_ins.get(&account.id);
    let cancels = sign_in.running.lock().expect("never poisoned").cancels;
    let _one = sign_in.starting.lock().await;
    let failure = |why: String| failed(StatusCode::INTERNAL_SERVER_ERROR, &why, &t.0);
    let program = codex::find(gathr::codex::copies::storage(&s.data).program()).map_err(failure)?;
    let folder = account.folder(&s.data);
    let (mine, mut asks) = mpsc::unbounded_channel();
    let previous = {
        let mut running = sign_in.running.lock().expect("never poisoned");
        if running.cancels != cancels {
            return Ok(Json(json!({"cancelled":true})));
        }
        running.asks.replace(mine.clone())
    };
    stop(previous).await;
    sign_in.show(
        &s.chats,
        json!({"state":"waiting", "paste":false, "device":device}),
    );
    let started = Instant::now();
    let connected = Client::start(&program, &folder).await;
    let mut client = match connected {
        Ok(client) => client,
        Err(why) => {
            finish(&s, &sign_in, &mine, &t.0, started, Some(&why), device).await;
            return Err(failure(why));
        }
    };
    let kind = if device {
        "chatgptDeviceCode"
    } else {
        "chatgpt"
    };
    let page = tokio::select! {
        p = client.request("account/login/start", json!({"type":kind})) => p,
        done = cancelled(&mut asks) => {
            close(&s, client).await;
            sign_in.show(&s.chats, Value::Null);
            if let Some(done) = done { let _ = done.send(()); }
            return Ok(Json(json!({"cancelled":true})));
        }
    };
    let page = match page {
        Ok(page) => page,
        Err(why) => {
            close(&s, client).await;
            finish(&s, &sign_in, &mine, &t.0, started, Some(&why), device).await;
            return Err(failure(why));
        }
    };
    let url = page[if device { "verificationUrl" } else { "authUrl" }]
        .as_str()
        .unwrap_or_default();
    let shown = json!({"state":"waiting", "url":url, "paste":false, "device":device, "user_code":page["userCode"]});
    sign_in.show(&s.chats, shown.clone());
    if !device && let Err(why) = gathr::platform::open_browser(url) {
        s.writer.record(
            Event::point("gathr.engine.sign_in.browser")
                .trace(&t.0)
                .error("open")
                .attr("why", why),
        );
    }
    let job = Job {
        trace: t.0,
        started,
        mine,
        sign_in: sign_in.clone(),
    };
    tokio::spawn(watch_codex(
        s.clone(),
        client,
        asks,
        page["loginId"].clone(),
        job,
        device,
    ));
    Ok(Json(shown))
}

async fn watch_codex(
    s: Arc<State>,
    mut client: Client,
    mut asks: mpsc::UnboundedReceiver<Ask>,
    id: Value,
    job: Job,
    device: bool,
) {
    let Job {
        trace,
        started,
        mine,
        sign_in,
    } = job;
    let why = loop {
        tokio::select! {
            next = client.next() => match next {
                Ok(v) if v["method"] == "account/login/completed" && v["params"]["loginId"] == id => {
                    break (v["params"]["success"] != true).then(|| v["params"]["error"].as_str().unwrap_or("Codex refused sign-in").to_string());
                }
                Ok(_) => {}
                Err(why) => break Some(why),
            },
            Some(Ask::Cancel(done)) = asks.recv() => {
                let _ = client.send("account/login/cancel", json!({"loginId":id})).await;
                close(&s, client).await;
                sign_in.show(&s.chats, Value::Null);
                let _ = done.send(());
                return;
            }
        }
    };
    close(&s, client).await;
    finish(&s, &sign_in, &mine, &trace, started, why.as_deref(), device).await;
}

async fn finish(
    s: &State,
    sign_in: &SignIn,
    mine: &mpsc::UnboundedSender<Ask>,
    trace: &str,
    started: Instant,
    why: Option<&str>,
    device: bool,
) {
    record_as(s, "sign_in", &sign_in.account, trace, why, started).await;
    if sign_in.release(mine) {
        let value = why.map_or(Value::Null, |why| json!({"state":"failed", "why":why, "ref":ids::reference(trace), "device":device}));
        sign_in.show(&s.chats, value);
    }
}

async fn close(s: &State, mut client: Client) {
    client.engine.close_input();
    tokio::select! {
        _ = client.engine.wait() => {},
        () = s.stopped() => { client.engine.close(CLOSE_GRACE).await; }
    }
}

pub async fn log_out(s: &State, folder: &std::path::Path) -> Result<(), String> {
    let program = codex::find(gathr::codex::copies::storage(&s.data).program())?;
    let mut client = Client::start(&program, folder).await?;
    let result = tokio::select! {
        r = client.request("account/logout", json!({})) => r.map(|_| ()),
        () = s.stopped() => Err(STOPPING.into()),
    };
    close(s, client).await;
    result
}
