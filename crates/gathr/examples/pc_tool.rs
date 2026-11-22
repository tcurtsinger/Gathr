//! A made-up tool on this PC, for the tests and for trying the Plugins screen: an MCP server over its
//! stdin and stdout, "Notes". It prints a line on stderr as it starts. With `--fail` it prints what a
//! tool that can't start would, and ends.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, ServiceExt, tool, tool_handler, tool_router};

#[derive(Clone)]
struct Notes {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Notes {
    #[tool(description = "Find notes by words")]
    async fn search_notes(&self) -> Result<CallToolResult, ErrorData> {
        let found = "Note 3: plant the beans";
        Ok(CallToolResult::success(vec![ContentBlock::text(found)]))
    }

    #[tool(description = "Say the key it was started with")]
    async fn read_key(&self) -> Result<CallToolResult, ErrorData> {
        let key = std::env::var("NOTES_KEY").unwrap_or_default();
        // Its process too, for a test to see it end.
        let said = format!("NOTES_KEY is {key} in process {}", std::process::id());
        Ok(CallToolResult::success(vec![ContentBlock::text(said)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Notes {
    fn get_info(&self) -> ServerConfig {
        let caps = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(caps).with_server_info(Implementation::new("Notes", "1.0.0"))
    }
}

#[tokio::main]
async fn main() {
    if std::env::args().any(|a| a == "--fail") {
        eprintln!("notes: starting");
        eprintln!(r"notes: D:\Notes isn't a folder of notes");
        std::process::exit(1);
    }
    eprintln!("notes: ready");
    let notes = Notes {
        tool_router: Notes::tool_router(),
    };
    if let Ok(running) = notes.serve(rmcp::transport::stdio()).await {
        let _ = running.waiting().await;
    }
}
