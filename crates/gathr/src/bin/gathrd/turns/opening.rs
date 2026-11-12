//! A turn's start: the record of what it was given, and the turn's state from then on.

use std::collections::HashMap;
use std::time::Instant;

use gathr::claude::login;
use gathr::event::Event;
use gathr::ids::{now_us, span_id, ulid};
use gathr::store;
use serde_json::json;

use super::{Ids, Running, Task};

impl Task {
    /// The turn under way from now: its span starts here.
    pub(super) fn open_turn(&mut self, ids: Ids, gate: login::Hold, ask_chars: usize) {
        let span = span_id();
        let assigned = self.assigned.as_ref();
        if let Some(pass) = &self.pass {
            pass.on_turn(crate::tools::OnTurn {
                trace: ids.trace.clone(),
                turn: ids.turn.clone(),
                task: ids.task,
            });
        }
        // What Gathr gave the turn, in characters with a rough token estimate (0020: the engines
        // report totals only, so the parts are Gathr's own count, labelled an estimate).
        let briefing = self.pass.as_ref().map_or(0, |p| p.briefing_chars);
        let estimate = |chars: usize| chars.div_ceil(4);
        self.record(
            Event::point(&format!("invoke_agent {}", self.engine_name()))
                .attr("gathr.context.briefing.chars", briefing)
                .attr("gathr.context.briefing.tokens_estimate", estimate(briefing))
                .attr("gathr.context.ask.chars", ask_chars)
                .attr("gathr.context.ask.tokens_estimate", estimate(ask_chars))
                .attr("gathr.tools.off", crate::tools::briefing::off())
                .trace(&ids.trace)
                .chat(&ids.chat)
                .span("start", &span)
                .attr("gen_ai.operation.name", "invoke_agent")
                .attr("gen_ai.agent.id", ids.agent.clone())
                .attr(
                    "gen_ai.request.model",
                    assigned.and_then(|a| a.model.clone()),
                )
                .attr("gathr.effort", assigned.and_then(|a| a.effort.clone()))
                .attr("gathr.approvals", assigned.map(|a| a.approvals.as_str()))
                .attr("gathr.account", assigned.map(|a| a.account.clone()))
                .attr("gathr.turn.id", ids.turn.clone()),
        );
        self.turn = Some(Running {
            ids,
            span,
            sent_us: now_us(),
            partial: String::new(),
            partial_units: 0,
            streamed: false,
            tools: HashMap::new(),
            thought: None,
            thought_began: None,
            approvals: HashMap::new(),
            service_approvals: HashMap::new(),
            push_allowed: false,
            stopping: false,
            carried: false,
            stop_why: None,
            save_failed: None,
            signed_out: false,
            tools_started: 0,
            engine_began: false,
            said: false,
            compacted: false,
            limited: false,
            progress_at: Instant::now(),
            stalled: false,
            gate,
        });
    }

    /// A turn the engine began itself, to answer a message sent into the last one.
    pub(super) async fn begin_read_turn(&mut self, message: &str, ask_chars: usize) {
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace: gathr::ids::trace_id(),
            agent: self.agent.clone(),
            task: None,
            answers: None,
            person: true,
        };
        let (row, id) = (ids.clone(), message.to_string());
        let saved = self
            .chats
            .writer
            .call(move |conn| {
                store::add_turn(
                    conn,
                    &row.chat,
                    &row.turn,
                    &row.trace,
                    (row.agent.as_deref(), row.task, row.answers.as_deref()),
                )?;
                store::move_message(conn, &id, &row.turn)
            })
            .await;
        // An open window moves the message to its new turn, where its reply or failure shows.
        if let Some(Ok(message)) = &saved {
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        }
        if let Some(Err(e)) = saved {
            let event = Event::point("gathr.turn.read")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
        self.open_turn(ids.clone(), login::Hold::default(), ask_chars);
        if let Some(turn) = self.turn.as_mut() {
            turn.engine_began = true;
        }
        self.set_turn(&ids, "running", None).await;
    }
}
