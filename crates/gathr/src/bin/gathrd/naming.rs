//! The background model: a small, quick model on one of the person's own accounts, for work nobody
//! waits on. Its one job: a moment after a new chat's first message, a short name for the chat.
//! A failure shows nothing in the window: it is one `gathr.background.name` event.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Extension, State as Extract};
use gathr::accounts::Account;
use gathr::claude::engine::{Engine, Line};
use gathr::claude::login::Login;
use gathr::event::Event;
use gathr::titles;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::Answer;

/// How long a run may take before its answer is let go.
const LIMIT: Duration = Duration::from_secs(30);

/// A model of a signed-in account, as the account lists it.
struct Choice {
    account: Account,
    model: String,
    name: String,
    /// The exact model a Codex Latest means today.
    resolved: Option<String>,
}

impl Choice {
    fn shown(&self) -> Value {
        json!({ "account": self.account.id, "account_name": self.account.name,
            "model": self.model, "name": self.name })
    }

    /// Gathr's own pick: Claude Code's small model, by its name (`haiku`). Codex names none so.
    fn small(&self) -> bool {
        self.account.engine == "claude" && self.model.contains("haiku")
    }

    /// The model the engine is given: none for its default.
    fn given(&self) -> Option<&str> {
        let latest = self.model.starts_with(gathr::codex::models::LATEST);
        match self.model.as_str() {
            "default" => None,
            _ if latest => self.resolved.as_deref(),
            model => Some(model),
        }
    }
}

/// Every model of every signed-in account, accounts in the order they were added.
async fn choices(s: &State, trace: &str) -> Vec<Choice> {
    let listed = s.writer.call(|c| gathr::accounts::list(c)).await;
    let mut all = Vec::new();
    for account in listed.and_then(Result::ok).unwrap_or_default() {
        let login = s.chats.login_status(&account.folder(&s.data)).await.0;
        if !matches!(login, Login::SignedIn | Login::Stale) {
            continue;
        }
        let Ok(list) = crate::models::list(s, trace, &account).await else {
            continue;
        };
        for m in list["models"].as_array().into_iter().flatten() {
            let Some(model) = m["id"].as_str() else {
                continue;
            };
            all.push(Choice {
                account: account.clone(),
                model: model.to_string(),
                name: m["name"].as_str().unwrap_or(model).to_string(),
                resolved: m["resolved"].as_str().map(String::from),
            });
        }
    }
    all
}

/// Settings' choice: `{"auto", "choices"}`, Gathr's own pick (null when there's none) and every
/// model of every signed-in account, each `{"account", "account_name", "model", "name"}`.
pub async fn listed(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let all = choices(&s, &t.0).await;
    let auto = all.iter().find(|c| c.small()).map(Choice::shown);
    let shown: Vec<Value> = all.iter().map(Choice::shown).collect();
    Ok(Json(json!({ "auto": auto, "choices": shown })))
}

/// The model the background runs on: the person's pick while its account is signed in and lists
/// it, else, with no pick, Gathr's own; none when nothing fits.
async fn model(s: &State, trace: &str) -> Option<Choice> {
    let picked = s.writer.call(|c| titles::model(c)).await?.ok()?;
    let mut all = choices(s, trace).await.into_iter();
    match picked {
        Some((account, model)) => all.find(|c| c.account.id == account && c.model == model),
        None => all.find(Choice::small),
    }
}

/// After the person's message `message` is saved: names the chat when it was its first, apart from
/// the send, which it never holds back.
pub fn after_send(s: &Arc<State>, chat: &str, message: &str, trace: &str) {
    let (s, chat, message) = (s.clone(), chat.to_string(), message.to_string());
    let trace = trace.to_string();
    tokio::spawn(async move { name(&s, &chat, &message, &trace).await });
}

async fn name(s: &State, chat: &str, message: &str, trace: &str) {
    let (id, said) = (chat.to_string(), message.to_string());
    let first = s.writer.call(move |c| titles::first(c, &id, &said)).await;
    // Not the chat's first message, or its title isn't its first line's: one try per chat.
    let Some(Ok(Some((words, was)))) = first else {
        return;
    };
    let started = Instant::now();
    let mut event = Event::point("gathr.background.name")
        .trace(trace)
        .chat(chat);
    let outcome = if words.chars().count() < titles::SHORTEST {
        Err("skipped: the message is short".to_string())
    } else {
        match model(s, trace).await {
            None => Err("skipped: there is no background model".to_string()),
            Some(choice) => {
                event = event
                    .attr("account", choice.account.id.clone())
                    .attr("gen_ai.request.model", choice.model.clone());
                named(s, chat, &was, &choice, &words).await
            }
        }
    };
    event.duration_us = Some(started.elapsed().as_micros() as i64);
    let event = match outcome {
        Ok(()) => {
            s.chats
                .publish(json!({ "type": "chat", "chat": chat, "change": "named" }));
            event.ok().attr("outcome", "named")
        }
        Err(why) => match why.split_once(": ") {
            Some((outcome, why)) => event
                .error(outcome)
                .attr("outcome", outcome)
                .attr("why", why),
            None => event
                .error("failed")
                .attr("outcome", "failed")
                .attr("why", why),
        },
    };
    s.writer.record(event);
}

/// One run, and its answer as the chat's name: `Err` is the outcome, `: `, and why, never the words.
async fn named(
    s: &State,
    chat: &str,
    was: &str,
    choice: &Choice,
    words: &str,
) -> Result<(), String> {
    let said = run(s, choice, words)
        .await
        .map_err(|why| format!("failed: {why}"))?;
    let Some(title) = titles::answer(&said) else {
        return Err("failed: the answer wasn't a name".into());
    };
    let (id, was) = (chat.to_string(), was.to_string());
    match s
        .writer
        .call(move |c| titles::name(c, &id, &was, &title))
        .await
    {
        Some(Ok(true)) => Ok(()),
        Some(Ok(false)) => Err("skipped: the chat was renamed".into()),
        _ => Err("failed: the name wasn't saved".into()),
    }
}

/// One prompt on the choice's account and model, through its engine's one-shot mode, in an empty
/// folder of Gathr's own: what it printed, or why not. The message goes on its input, never in its
/// arguments, which any process on the computer can read.
async fn run(s: &State, choice: &Choice, words: &str) -> Result<String, String> {
    let codex = choice.account.engine == "codex";
    let program = match codex {
        true => gathr::codex::command::find(gathr::codex::copies::storage(&s.data).program()),
        false => gathr::claude::command::find(s.chats.copy()),
    }?;
    let folder = choice.account.folder(&s.data);
    let cwd = s.data.path().join("background");
    std::fs::create_dir_all(&cwd).map_err(|e| format!("its folder wasn't made: {e}"))?;
    // The environment an engine gets: the person's now, without their own logins (G23).
    let vars = gathr::run::fresh().await.into_iter();
    let (args, env) = match codex {
        true => (
            gathr::codex::command::exec(choice.given(), titles::ASK),
            gathr::codex::command::env(vars, &folder),
        ),
        false => (
            gathr::claude::command::one_shot(choice.given(), titles::ASK),
            gathr::claude::command::env(vars, &folder),
        ),
    };
    // It runs on the account's login, so it takes the renewal gate like any start (0002).
    let gate = crate::sign_in::pass_gate(s, &folder).await?;
    let (mut engine, mut lines) = Engine::start(&program, &args, env, &cwd)
        .map_err(|e| format!("it didn't start from {}: {e}", program.display()))?;
    let written = engine.write_line(words).await;
    engine.close_input();
    let read = async {
        let mut out = String::new();
        while let Some(line) = lines.recv().await {
            if let Line::Out(line) = line {
                out.push_str(&line);
                out.push('\n');
            }
        }
        (out, engine.wait().await)
    };
    let ended = tokio::select! {
        ended = tokio::time::timeout(LIMIT, read) => ended.ok(),
        () = s.stopped() => None,
    };
    drop(gate);
    let Some((out, code)) = ended else {
        // Let go, never killed: it may be renewing the login. Shutdown ends it after its grace.
        crate::models::close_later(s, engine, "gathr.background.close");
        return Err(format!("no answer in {} s", LIMIT.as_secs()));
    };
    written.map_err(|e| format!("its input wasn't written: {e}"))?;
    match code {
        Some(0) => Ok(out),
        code => Err(format!("it exited with {code:?}")),
    }
}
