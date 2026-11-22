//! What Codex asks the person, and their answers: approvals of a command, a file change or an MCP
//! tool call. Anything else it asks of a client is refused.

use serde_json::{Value, json};

use super::{Read, Session};
use crate::claude::frames::Frame;

impl Session {
    /// The person's answer to an approval Codex asked for.
    pub fn approval(&mut self, request_id: &str, allow: bool) -> Option<Value> {
        let (id, elicited) = self.approvals.remove(request_id)?;
        let decision = if allow { "accept" } else { "decline" };
        Some(match elicited {
            true => json!({ "id": id, "result": { "action": decision, "content": {} } }),
            false => json!({ "id": id, "result": { "decision": decision } }),
        })
    }

    pub(super) fn server_request(
        &mut self,
        method: &str,
        id: &Value,
        params: &Value,
        read: &mut Read,
    ) {
        let request_id = id.to_string();
        let asked = |tool: &str, input: Value| Frame::ApprovalAsked {
            request_id: request_id.clone(),
            tool: tool.to_string(),
            input,
            why: params["reason"].as_str().map(String::from),
        };
        match method {
            "item/commandExecution/requestApproval" => {
                let input = json!({ "command": params["command"], "cwd": params["cwd"] });
                read.frames.push(asked("Shell", input));
            }
            "item/fileChange/requestApproval" => {
                let input = json!({ "grantRoot": params["grantRoot"] });
                read.frames.push(asked("Edit", input));
            }
            // How Codex asks before an MCP tool call (its app-server schema, 0.166.0): a form with
            // nothing to fill in. One that asks for fields or opens a page is no approval: refused.
            "mcpServer/elicitation/request" if approves(params) => {
                let server = params["serverName"].as_str().unwrap_or_default();
                let call = self.mcp_call.clone();
                let call = call.filter(|(c, _)| c.starts_with(&format!("mcp__{server}__")));
                // Its arguments, as Claude Code's ask carries them: the card shows a command.
                let (tool, input) = call.unwrap_or_else(|| (format!("mcp__{server}"), json!({})));
                read.frames.push(Frame::ApprovalAsked {
                    request_id: request_id.clone(),
                    tool,
                    input,
                    why: params["message"].as_str().map(String::from),
                });
                self.approvals.insert(request_id, (id.clone(), true));
                return;
            }
            // Nothing else Codex asks of a client is Gathr's to give: refused, so it never waits.
            _ => {
                read.send.push(json!({ "id": id, "error": { "code": -32601,
                    "message": format!("Gathr doesn't answer {method}") } }));
                read.frames.push(Frame::Other(method.to_string()));
                return;
            }
        }
        self.approvals.insert(request_id, (id.clone(), false));
    }
}

/// An elicitation that only asks yes or no: a form with no fields.
fn approves(params: &Value) -> bool {
    let fields = &params["requestedSchema"]["properties"];
    params["mode"] == "form" && fields.as_object().is_none_or(|f| f.is_empty())
}
