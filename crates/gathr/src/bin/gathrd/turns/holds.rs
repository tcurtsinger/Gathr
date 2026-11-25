//! What keeps an agent's turns from starting, and the person's Stop now (0010, F10): a pause holds
//! the turns Gathr queued (the person's own message only if it was queued before Stop now), a usage
//! limit holds the account's turns until 30 s after its reset, and Stop now ends what runs in a
//! scope with a carry-on that waits for Restart. Also the turn Claude Code starts by itself when a background command ends,
//! and the flag on a turn that makes no progress.

use std::time::{Duration, Instant};

use gathr::claude::frames;
use gathr::event::Event;
use gathr::ids::{now_us, span_id, ulid};
use gathr::pauses::{self, Pause};
use gathr::store;
use gathr::store::Start;
use serde_json::json;

use super::{Ids, Running, Task};

/// What a turn that a usage limit cut off partway, or Stop now, says when it carries on.
use gathr::carry::CHECK_FIRST;
use gathr::words::Opened;

/// Who stops a scope's turns when the person presses Stop now.
pub const STOP_NOW: &str = "Stop now";

impl Running {
    /// For how long it has made no progress (text, or a tool starting or ending), once that is
    /// past the limit: the stall time, twice that while a tool runs. Never while an approval of it
    /// waits: it then waits on the person.
    pub(super) fn silent_for(&self, stall: Duration) -> Option<Duration> {
        let limit = match self.tools.is_empty() {
            true => stall,
            false => stall * 2,
        };
        let quiet = self.progress_at.elapsed();
        (quiet >= limit && !self.asks()).then_some(quiet)
    }
}

impl Task {
    /// What holds the queued turn from starting: the update the person asked for, installing now
    /// (0032), whose hand-over would cut it off; else a pause (0010), never the person's message.
    pub(super) fn holder(&self, ids: &Ids) -> Option<&'static str> {
        if self.chats.updating().is_some() {
            return Some("update");
        }
        if ids.person {
            return None;
        }
        let pauses = self.chats.pauses();
        pauses::holding(&pauses, &ids.chat).map(|_| "pause")
    }

    /// Marks each queued turn with what holds it: saved, and sent live when it changes.
    pub(super) async fn mark_holds(&mut self) {
        let usage = self.usage_wait.is_some();
        let mut changed = Vec::new();
        for (at, queued) in self.queue.iter().enumerate() {
            let on_usage = (usage && queued.held == Some("usage")).then_some("usage");
            let engine = queued.held.filter(|h| matches!(*h, "claude" | "codex"));
            let held = self.holder(&queued.ids).or(on_usage).or(engine);
            if held != queued.held {
                changed.push((at, held));
            }
        }
        for (at, held) in changed {
            self.queue[at].held = held;
            let ids = self.queue[at].ids.clone();
            self.save_held(&ids, held).await;
        }
    }

    pub(super) async fn save_held(&self, ids: &Ids, held: Option<&'static str>) {
        let why = match held {
            Some("pause") => Some("paused".to_string()),
            Some("claude") => Some("Claude Code is downloading".to_string()),
            Some("codex") => Some("Codex is downloading".to_string()),
            Some("update") => self.chats.updating().map(|v| format!("updating to {v}")),
            Some(_) => self.usage_words(),
            None => None,
        };
        let (turn, words) = (ids.turn.clone(), why.clone());
        let saved = self
            .chats
            .writer
            .call(move |c| store::set_held(c, &turn, held, words.as_deref()));
        if let Some(Err(e)) = saved.await {
            let event = Event::point("gathr.turn.held")
                .trace(&ids.trace)
                .error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
        let update = json!({ "type": "turn", "turn": ids.turn, "status": "queued", "held": held,
            "why": why, "agent": ids.agent, "answers": ids.answers });
        self.publish_turn(ids, update).await;
    }

    fn usage_words(&self) -> Option<String> {
        let until = self.usage_until_us?;
        Some(format!("out of usage until {}", gathr::ids::utc(until)))
    }

    /// Keeps the turn in the queue, marked as waiting on the account's usage: saved once.
    pub(super) async fn hold_on_usage(&mut self, next: &super::Queued) {
        let at = match self.queue.iter().position(|q| q.ids.turn == next.ids.turn) {
            Some(at) => at,
            None => {
                self.queue.push_front(super::Queued {
                    held: None,
                    ..next.clone()
                });
                0
            }
        };
        if self.queue[at].held != Some("usage") {
            self.queue[at].held = Some("usage");
            self.set_turn(&next.ids, "queued", None).await;
            self.save_held(&next.ids, Some("usage")).await;
        }
    }

    /// Until when the account's turns wait, if it's out of usage; starts the timer that tries again.
    pub(super) async fn usage_held(&mut self, account: &str) -> bool {
        let id = account.to_string();
        let until = self
            .chats
            .writer
            .call(move |c| pauses::usage_until(c, &id))
            .await;
        let Some(Ok(Some(until))) = until else {
            self.usage_wait = None;
            self.usage_until_us = None;
            return false;
        };
        let wait = Duration::from_micros((until - now_us()).max(0) as u64);
        self.usage_wait = Some(Instant::now() + wait);
        self.usage_until_us = Some(until);
        true
    }

    /// The engine said the account is out of usage: saved at once, so every turn on it waits.
    pub(super) async fn out_of_usage(&mut self, resets_at: i64, kind: Option<String>) {
        let Some(account) = self.assigned.as_ref().map(|a| a.account.clone()) else {
            return;
        };
        let trace = self
            .turn
            .as_ref()
            .map_or_else(gathr::ids::trace_id, |t| t.ids.trace.clone());
        if let Some(turn) = self.turn.as_mut() {
            turn.limited = true;
        }
        let until = resets_at * 1_000_000;
        let saved = self
            .chats
            .writer
            .call(move |c| pauses::hold_usage(c, &account, until, kind.as_deref(), &trace));
        if let Some(Err(e)) = saved.await {
            self.record(
                Event::point("gathr.usage.hold")
                    .error("sqlite")
                    .attr("why", e.to_string()),
            );
        }
        self.chats.settle_pauses(true).await;
    }

    /// A turn the usage limit ended: before any tool ran it's sent again once the account is
    /// back; after one ran, it ends and a carry-on waits instead, so nothing runs twice (F10).
    /// Answers whether it handled the end.
    pub(super) async fn limited_end(&mut self, ids: &Ids, tools_started: u32) -> bool {
        let why = gathr::ends::OUT_OF_USAGE;
        if tools_started == 0 {
            let text = self.text_of(&ids.turn).await;
            let queued = super::Queued {
                ids: ids.clone(),
                text,
                held: None,
            };
            self.queue.push_front(queued);
            self.set_turn(ids, "queued", Some(why)).await;
            return true;
        }
        let said = format!("The account's usage limit interrupted your turn. {CHECK_FIRST}");
        self.carry_on(ids, said).await;
        self.set_turn(ids, "stopped", Some(why)).await;
        true
    }

    /// Queues a turn that carries on from `ids` in the same session: for its agent, or for none in
    /// a chat sent to no agent. Whether it was saved.
    async fn carry_on(&mut self, ids: &Ids, text: String) -> bool {
        let (chat, agent) = (ids.chat.clone(), ids.agent.clone());
        let (answers, said) = (ids.turn.clone(), text.clone());
        let saved = self.chats.writer.call(move |c| {
            let on = Some(answers.as_str());
            Start::save(c, (&chat, agent.as_deref()), on, Opened::CarryOn, &said)
        });
        match saved.await {
            Some(Ok((turn, trace, message))) => {
                let carry = Ids {
                    chat: ids.chat.clone(),
                    turn,
                    trace,
                    agent: ids.agent.clone(),
                    answers: Some(ids.turn.clone()),
                    person: false,
                };
                self.queue_given(carry, text, message).await;
                true
            }
            Some(Err(e)) => {
                let event = Event::point("gathr.turn.carry_on")
                    .trace(&ids.trace)
                    .error("sqlite");
                self.record(event.attr("why", e.to_string()));
                false
            }
            None => false,
        }
    }

    /// Stop now (the person's alone): the running turn in the scope is interrupted, the engine's
    /// background commands there are ended by its own stop, and a carry-on naming them waits for
    /// Restart. The scope is already paused, stopped, so nothing else starts. An update stops the
    /// same way with its carry-ons saved before (`carried`: the turns they answer, `gathr::carry`):
    /// none is saved here for those. A turn the update saved none for (it started as the update
    /// began) is carried on as Stop now does.
    pub(super) async fn stop_now(
        &mut self,
        scope: String,
        target: String,
        by: &str,
        carried: Option<std::sync::Arc<std::collections::HashSet<String>>>,
    ) {
        let pause = [Pause {
            scope,
            target,
            since_us: 0,
            quiet_us: None,
            stopped: true,
        }];
        // The person's messages already queued in the scope wait for Restart too: only what they
        // send after the stop runs.
        for queued in &mut self.queue {
            if pauses::holding(&pause, &queued.ids.chat).is_some() {
                queued.ids.person = false;
            }
        }
        self.mark_holds().await;
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        if self.engine.is_none() || pauses::holding(&pause, &chat).is_none() {
            return;
        }
        let (c, agent) = (chat.clone(), self.agent.clone());
        let commands = self
            .chats
            .writer
            .call(move |conn| gathr::background::of(conn, &c, agent.as_deref()));
        let commands = commands.await.and_then(Result::ok).unwrap_or_default();
        for (id, _) in &commands {
            let request = frames::stop_task(&format!("stop-now-{}", span_id()), id);
            let _ = self.write(&request, None).await;
        }
        let running = self
            .turn
            .as_ref()
            .filter(|t| !t.stopping)
            .map(|t| t.ids.clone());
        let saved = |turn: &str| carried.as_ref().is_some_and(|c| c.contains(turn));
        let carried = match &running {
            Some(ids) => saved(&ids.turn),
            None => commands.iter().any(|(_, turn)| saved(turn)),
        };
        if running.is_some() {
            self.interrupt(&chat).await;
        }
        let turn_ended = running.is_some();
        let ids = running.or_else(|| {
            let (_, turn) = commands.first()?;
            let trace = gathr::ids::trace_id();
            let (agent, chat) = (self.agent.clone(), chat.clone());
            Some(Ids {
                chat,
                turn: turn.clone(),
                trace,
                agent,
                answers: None,
                person: false,
            })
        });
        let Some(ids) = ids else { return };
        if !carried {
            let ended = gathr::carry::ended(turn_ended, commands.len());
            self.carry_on(&ids, gathr::carry::said(by, &ended)).await;
        }
        self.record(
            Event::point("gathr.pause.stop_now")
                .trace(&ids.trace)
                .chat(&chat)
                .ok()
                .attr("gathr.background.count", commands.len()),
        );
    }

    /// The engine's background commands, as it lists them now: saved under the running turn, and
    /// the turn of one that ended remembered for the turn the engine starts next by itself.
    pub(super) async fn background(&mut self, listed: Vec<gathr::background::Listed>) {
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        self.background = listed.len();
        let (agent, turn) = (
            self.agent.clone(),
            self.turn.as_ref().map(|t| t.ids.turn.clone()),
        );
        // Idle, the engine begins a turn of its own once a command ends: it answers that command's.
        let idle = turn.is_none();
        let c = chat.clone();
        let saved = self.chats.writer.call(move |conn| {
            let engine = (c.as_str(), agent.as_deref());
            let ended = gathr::background::set(conn, engine, &listed, turn.as_deref())?;
            Ok::<_, gathr::Error>((ended, gathr::background::in_chat(conn, &c)?))
        });
        match saved.await {
            Some(Ok((ended, running))) => {
                if let Some(last) = ended.into_iter().last().filter(|_| idle) {
                    self.woke_from = Some(last);
                }
                self.publish(
                    &chat,
                    json!({ "type": "background", "background": running }),
                );
                crate::chat_state::changed(&self.chats, &chat, &gathr::ids::trace_id()).await;
            }
            Some(Err(e)) => self.record(
                Event::point("gathr.engine.background")
                    .error("sqlite")
                    .attr("why", e.to_string()),
            ),
            None => {}
        }
        self.chats.settle_pauses(false).await;
    }

    /// The engine said how a background command ended and where its output went: kept on its row
    /// until the engine no longer lists it.
    pub(super) async fn task_ended(&mut self, id: String, status: String, output: Option<String>) {
        let told = self
            .chats
            .writer
            .call(move |conn| gathr::background::told(conn, &id, &status, output.as_deref()));
        if let Some(Err(e)) = told.await {
            self.record(
                Event::point("gathr.engine.background")
                    .error("sqlite")
                    .attr("why", e.to_string()),
            );
        }
    }

    /// The engine in `chat` exited, on a close or on its own: its background commands ended with
    /// it, so a pause stops waiting on them.
    pub(super) async fn engine_gone(&mut self, chat: String) {
        self.background = 0;
        self.woke_from = None;
        if let Some(agent) = &self.agent {
            crate::cleanup::engine_closed(&self.chats, &chat, agent).await;
        }
        let (agent, c) = (self.agent.clone(), chat.clone());
        let gone = self.chats.writer.call(move |conn| {
            gathr::background::gone(conn, &c, agent.as_deref())?;
            gathr::background::in_chat(conn, &c)
        });
        match gone.await {
            Some(Ok(running)) => {
                self.publish(
                    &chat,
                    json!({ "type": "background", "background": running }),
                );
                crate::chat_state::changed(&self.chats, &chat, &gathr::ids::trace_id()).await;
            }
            Some(Err(e)) => self.record(
                Event::point("gathr.engine.background")
                    .error("sqlite")
                    .attr("why", e.to_string()),
            ),
            None => {}
        }
        self.chats.settle_pauses(false).await;
    }

    /// Claude Code began a turn by itself (a background command ended): it gets its row, answering
    /// the turn that started the command, so its answer is kept.
    pub(super) async fn begin_woken_turn(&mut self) {
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace: gathr::ids::trace_id(),
            agent: self.agent.clone(),
            answers: self.woke_from.take(),
            person: false,
        };
        let row = ids.clone();
        let saved = self.chats.writer.call(move |c| {
            store::add_turn(
                c,
                &row.chat,
                &row.turn,
                &row.trace,
                (row.agent.as_deref(), row.answers.as_deref()),
                Opened::Engine,
            )
        });
        if let Some(Err(e)) = saved.await {
            let event = Event::point("gathr.turn.woke")
                .trace(&ids.trace)
                .error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
        self.record(
            Event::point("gathr.turn.woke")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .ok(),
        );
        self.open_turn(ids.clone(), gathr::claude::login::Hold::default(), 0);
        if let Some(turn) = self.turn.as_mut() {
            turn.engine_began = true;
        }
        self.set_turn(&ids, "running", None).await;
    }

    /// A turn that goes silent is flagged, once, in the event record: never stopped (F10).
    pub(super) fn check_stall(&mut self) {
        let stall = self.times.stall;
        let Some(turn) = self.turn.as_mut().filter(|t| !t.stalled) else {
            return;
        };
        let Some(quiet) = turn.silent_for(stall) else {
            return;
        };
        turn.stalled = true;
        let ids = turn.ids.clone();
        let quiet_ms = quiet.as_millis() as i64;
        let event = Event::point("gathr.turn.stalled")
            .trace(&ids.trace)
            .chat(&ids.chat);
        self.record(
            event
                .error("no_progress")
                .attr("quiet_ms", quiet_ms)
                .attr("gen_ai.agent.id", ids.agent),
        );
    }
}
