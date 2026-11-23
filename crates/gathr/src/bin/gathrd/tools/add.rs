//! `add_tool` and `add_skill`: an agent adds a tool or writes a skill when the person asks. Unlike
//! Gathr's other tools, an engine asks about them as the agent's Approvals say (`ASKED_TOOLS`).
//! Each answer's first lines are what the chat shows under the agent's message: what it did, the
//! name, then for a tool its id and what it needs from the person.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::http::request::Parts;
use gathr::pc_tools;
use gathr::web_tools::{self, Access, Problem};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::Caller;
use super::server::{Tools, caller_of};
use crate::api::{State, Trace};

/// What every answer ends with: an engine is given tools and skills as it starts, and an agent's next
/// turn starts its engine again once they have changed.
const NEXT_START: &str = "Agents get it from their next turn. You can't use it in this one.";

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddTool {
    /// A tool on the web: its link.
    link: Option<String>,
    /// A tool on this PC: the command line or the JSON block from its instructions, as the person
    /// would paste it. Give a secret's value as "" for the person to fill in.
    command: Option<String>,
    /// Its name, if not the one it gives itself.
    name: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddSkill {
    /// The skill's folder, which you wrote in your own folder or Shared: its name is the skill's, and
    /// it holds a SKILL.md with `name` and `description` in its front matter.
    folder: String,
    /// True to replace the skill of that name.
    replace: Option<bool>,
}

#[tool_router(router = add_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "When the person asks for a tool (an MCP server), add it for every agent: its link, or the \
                       command from its instructions. Leave a secret's value empty: the person fills it in."
    )]
    async fn add_tool(
        &self,
        Parameters(ask): Parameters<AddTool>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let name = ask.name;
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let added = tool(&self.state, &trace, (ask.link, ask.command), name).await;
        let added = added.map(|(answer, _)| answer);
        Ok(self.respond(&caller, "add_tool", started, added).await)
    }

    #[tool(
        description = "When the person asks for a skill, write its folder in your own folder (a SKILL.md with name and \
                       description in its front matter, and any other files), then add it here for every agent."
    )]
    async fn add_skill(
        &self,
        Parameters(ask): Parameters<AddSkill>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let added = own_skill(&self.state, &caller, &ask.folder, ask.replace == Some(true)).await;
        Ok(self.respond(&caller, "add_skill", started, added).await)
    }
}

/// A tool on the web from its link, or on this PC from its command: one of them. The answer, and the tool's id.
pub(super) async fn tool(
    s: &Arc<State>,
    trace: &str,
    (link, command): (Option<String>, Option<String>),
    name: Option<String>,
) -> Result<(String, String), String> {
    let name = name.map(|n| one_line(&n)).filter(|n| !n.is_empty());
    let given = |text: Option<String>| text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    match (given(link), given(command)) {
        (Some(link), None) => on_web(s, link, name).await,
        (None, Some(command)) => on_pc(s, trace, command, name).await,
        _ => Err("Give a link or a command: one of them.".into()),
    }
}

/// A name as the chat's line shows it: on one line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The answer for a tool added: what it needs from the person, then how it went.
fn added(name: &str, id: &str, needs: Option<String>, went: String) -> String {
    let mut lines = vec![
        "Added the tool:".to_string(),
        one_line(name),
        format!("id {id}"),
    ];
    lines.extend(needs.map(|n| format!("It needs {n}, which the person gives it in Plugins.")));
    lines.extend([went, NEXT_START.to_string()]);
    lines.join("\n")
}

/// Its actions, as an answer names them.
fn actions(names: &[String]) -> String {
    match names {
        [] => "no actions".into(),
        names => format!("its actions {}", names.join(", ")),
    }
}

/// Runs `work` on the database.
async fn db<T: Send + 'static>(
    s: &State,
    work: impl FnOnce(&mut rusqlite::Connection) -> Result<T, gathr::Error> + Send + 'static,
) -> Result<T, String> {
    let done = s.writer.call(work).await;
    let done =
        done.unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())));
    done.map_err(|e| e.to_string())
}

/// A tool on the web, connected to as the person's Add does. One that wants a key or signs in through
/// the browser is saved without either: the person gives it in Plugins. No key is ever taken here.
async fn on_web(
    s: &Arc<State>,
    link: String,
    name: Option<String>,
) -> Result<(String, String), String> {
    let host = web_tools::checked(&link).map_err(|e| e.to_string())?;
    let same = link.clone();
    let taken = db(s, move |c| {
        let tools = web_tools::list(c)?;
        Ok(tools
            .into_iter()
            .find(|t| t.link.trim() == same)
            .map(|t| t.name))
    });
    if let Some(name) = taken.await? {
        return Err(format!("{name} is already added with this link."));
    }
    let (named, needs, went) =
        match web_tools::connect(&link, &Access::Open, web_tools::CONNECT).await {
            Ok(remote) => {
                let names = web_tools::actions(&remote, web_tools::CONNECT).await;
                let names: Vec<String> = names
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| a.name)
                    .collect();
                let went = format!("It works, with {}.", actions(&names));
                (web_tools::own_name(&remote), None, went)
            }
            Err(Problem::NeedsKey) => (
                None,
                Some("a key".to_string()),
                "It's saved without one.".into(),
            ),
            Err(Problem::SignIn) => (
                None,
                Some("a sign-in".to_string()),
                "It's saved without one.".into(),
            ),
            Err(_) => return Err("No tool at this link.".into()),
        };
    let name = name.or(named).unwrap_or(host);
    let saved = db(s, move |c| web_tools::add(c, &name, &link, None)).await?;
    let _ = s.chats.live.send(json!({ "type": "tools" }));
    Ok((added(&saved.name, &saved.id, needs, went), saved.id))
}

/// A tool on this PC, saved and started as the person's Add does. A variable given empty is the
/// person's to fill in: the tool is saved all the same.
async fn on_pc(
    s: &Arc<State>,
    trace: &str,
    command: String,
    name: Option<String>,
) -> Result<(String, String), String> {
    let runs = pc_tools::read(&command).map_err(|e| e.to_string())?.runs;
    let empty: Vec<String> = (runs.variables.iter())
        .filter(|(_, value)| value.is_empty())
        .map(|(name, _)| name.clone())
        .collect();
    // The same command with other variables is another tool: one server run once per league, say. A variable given
    // empty is the person's to fill in, so it matches whatever the saved one holds.
    let asked = runs.clone();
    let taken = db(s, move |c| {
        for tool in web_tools::list(c)?.into_iter().filter(|t| t.kind == "pc") {
            let Ok(was) = pc_tools::saved(c, &tool.id) else {
                continue;
            };
            let command = (&was.program, &was.args, &was.folder)
                == (&asked.program, &asked.args, &asked.folder);
            let variables = was.variables.len() == asked.variables.len()
                && (asked.variables.iter()).all(|(name, value)| {
                    was.variables
                        .get(name)
                        .is_some_and(|w| value.is_empty() || value == w)
                });
            if command && variables {
                return Ok(Some(tool.name));
            }
        }
        Ok(None)
    });
    if let Some(name) = taken.await? {
        return Err(format!(
            "{name} already runs this command with the same variables."
        ));
    }
    let trace = Trace(trace.to_string());
    let saved = crate::pc_tools::add(s, &trace, &command, name).await;
    let saved = saved.map_err(|fail| fail.why().to_string())?.0;
    let (id, name) = (saved["tool"]["id"].as_str(), saved["tool"]["name"].as_str());
    let (id, name) = (id.unwrap_or_default(), name.unwrap_or_default());
    let went = match crate::pc_tools::outcome(s, id).await {
        Ok(names) => format!("It's running, with {}.", actions(&names)),
        Err(Some(printed)) => format!("It didn't start. It printed:\n{printed}"),
        Err(None) => "It didn't start, and printed nothing.".into(),
    };
    let needs = (!empty.is_empty()).then(|| format!("a value for {}", empty.join(" and ")));
    Ok((added(name, id, needs, went), id.to_string()))
}

/// A skill from a folder the agent wrote, inside what it may write, links followed.
async fn own_skill(
    s: &State,
    caller: &Caller,
    folder: &str,
    replace: bool,
) -> Result<String, String> {
    caller.agent.as_ref().ok_or("You're not an agent here.")?;
    let path = caller.cwd.join(folder.trim());
    let inside = gathr::approvals::within(&path, &caller.folders);
    let real = std::fs::canonicalize(&path).ok().filter(|p| p.is_dir());
    let Some(real) = real.filter(|_| inside) else {
        return Err(format!(
            "{folder} isn't a folder in your own folder or Shared."
        ));
    };
    skill(s, real, replace).await.map(|(answer, _)| answer)
}

/// A skill copied from the folder `real`. The answer, and the skill's name.
pub(super) async fn skill(
    s: &State,
    real: PathBuf,
    replace: bool,
) -> Result<(String, String), String> {
    let data: PathBuf = s.data.path().to_path_buf();
    let copied = db(s, move |c| {
        let dir = gathr::skills::folder(c, &data)?;
        gathr::skills::add_folder(&dir, &real, replace)
    });
    let (name, replaced) = copied.await?;
    let _ = s.chats.live.send(json!({ "type": "skills" }));
    let did = if replaced {
        "Changed the skill:"
    } else {
        "Wrote the skill:"
    };
    Ok((format!("{did}\n{name}\n{NEXT_START}"), name))
}
