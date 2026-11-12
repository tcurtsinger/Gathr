//! `merge` (0043 C4): a PR's reviewer asks Gathr to merge it; agents never merge themselves.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Merge {
    /// The PR you reviewed, by number.
    pr: i64,
    /// Yes when Gathr asked for your sign-off (the PR changes tests or check settings, or brings
    /// the project its first checks) and you checked they still test the code.
    signed_off: Option<bool>,
}

#[tool_router(router = merge_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Reviewer only: merge a PR you reviewed. Gathr merges it once your review is done on its head, \
                       its checks passed there and it's up to date with its base, else says why. Never merge another way."
    )]
    async fn merge(
        &self,
        Parameters(ask): Parameters<Merge>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let merged =
            crate::merging::merge(&self.chats, &caller, ask.pr, ask.signed_off == Some(true)).await;
        Ok(self.respond(&caller, "merge", started, merged).await)
    }
}
