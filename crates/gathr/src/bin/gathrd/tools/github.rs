//! `pr`, `ci_failures` and `update_pr` (0046, G185): GitHub as Gathr reaches it, for any engine,
//! with no GitHub login inside the engine's sandbox. Merging stays `merge`.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Pr {
    /// The PR, by number, in this chat's project.
    pr: i64,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePr {
    /// The PR you review, by number.
    pr: i64,
    /// Its head as you last read it (at least 7 characters): the update is refused if it moved.
    head: String,
}

#[tool_router(router = github_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "A PR in this chat's project as GitHub has it: state, head, base, mergeable, review, and each check \
                       on its head. Gathr asks GitHub with its own sign-in: no `gh` login is needed where you run."
    )]
    async fn pr(
        &self,
        Parameters(ask): Parameters<Pr>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let read = crate::github::pr(&self.chats, &caller, ask.pr).await;
        Ok(self.respond(&caller, "pr", started, read).await)
    }

    #[tool(
        description = "Why a PR's CI failed: for each failed check on its head that ran on GitHub Actions, its log's last \
                       lines and its annotations, secrets hidden."
    )]
    async fn ci_failures(
        &self,
        Parameters(ask): Parameters<Pr>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let read = crate::github::failures(&self.chats, &caller, ask.pr).await;
        Ok(self.respond(&caller, "ci_failures", started, read).await)
    }

    #[tool(
        description = "Reviewer only: bring a PR you review up to date with its base, as GitHub's Update branch does. \
                       Give its head as you read it: Gathr refuses a head that moved, a closed PR or one not into the \
                       default branch, and says the head before and after. Then wait for CI on the new head."
    )]
    async fn update_pr(
        &self,
        Parameters(ask): Parameters<UpdatePr>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let updated = crate::github::update(&self.chats, &caller, ask.pr, &ask.head).await;
        Ok(self.respond(&caller, "update_pr", started, updated).await)
    }
}
