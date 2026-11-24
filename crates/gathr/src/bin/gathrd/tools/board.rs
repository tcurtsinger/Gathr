//! `set_board_line`: an agent adds, changes or takes off a line of its chat's board. The window hears the
//! board whole. An agent's line still doing when the chat goes quiet hands it its turn back once (`gathr::nudge`).

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    ToDo,
    Doing,
    InReview,
    Done,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SetBoardLine {
    /// The line's number, to change it or take it off; none adds a new line.
    line: Option<i64>,
    /// What the piece of work is, in a few words.
    text: Option<String>,
    /// The agent in this chat who does it, by name; a new line is yours unless you name another.
    owner: Option<String>,
    /// A new line starts at to_do. in_review means a teammate is looking at it.
    state: Option<State>,
    /// True takes the line off the board.
    #[serde(default)]
    remove: bool,
}

#[tool_router(router = board_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Add, change or take off one line of this chat's board: a piece of work, its owner and its \
                       state (to_do, doing, in_review, done). The board is what the person sees of where the work is."
    )]
    async fn set_board_line(
        &self,
        Parameters(ask): Parameters<SetBoardLine>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, by) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let change = gathr::board::Change {
            line: ask.line,
            text: ask.text,
            owner: ask.owner,
            state: ask.state.map(|s| match s {
                State::ToDo => "to_do",
                State::Doing => "doing",
                State::InReview => "in_review",
                State::Done => "done",
            }),
            remove: ask.remove,
        };
        let set = self
            .chats
            .writer
            .call(move |conn| {
                let tx = conn.transaction()?;
                let lines = gathr::board::set(&tx, &chat, &by, change)?;
                tx.commit()?;
                Ok::<_, gathr::Error>(lines)
            })
            .await
            .unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())));
        let said = set.map(|lines| {
            let said = gathr::board::said(&lines);
            let event = json!({ "type": "board", "chat": caller.chat, "lines": lines });
            self.chats.publish(event);
            said
        });
        let said = said.map_err(|e| e.to_string());
        Ok(self.respond(&caller, "set_board_line", started, said).await)
    }
}
