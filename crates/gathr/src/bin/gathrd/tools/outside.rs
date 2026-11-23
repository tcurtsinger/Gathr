//! Gathr's tools for an AI app outside Gathr on this PC, at `/api/mcp`: the app runs `gathr connect`, which passes its
//! MCP messages here with the service's token, so this endpoint is guarded as `/api` is, never by an engine's token.
//! Each tool does what the window or an agent does, through the same code, with no approval in Gathr: the app's own
//! permission prompt is the gate. What it adds says which app added it (`gathr::added_by`), by the name `gathr connect`
//! sends in `x-gathr-app`.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use axum::http::request::Parts;
use gathr::event::Event;
use gathr::team::Role;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde_json::{Value, json};

use super::add;
use super::propose::{drafted, listed_model, project_name, team};
use crate::api::{State, Trace};

pub const PATH: &str = "/api/mcp";
/// The header `gathr connect` names the app in.
pub const APP: &str = "x-gathr-app";

/// The service to mount at `/api/mcp`: stateless, so `gathr connect` only passes each message on, and a restarted
/// service carries on where the last one was.
pub fn service(state: Arc<State>) -> StreamableHttpService<Outside, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_legacy_session_mode(false);
    let tools = move || {
        Ok(Outside {
            state: state.clone(),
            tool_router: Outside::tool_router(),
        })
    };
    StreamableHttpService::new(tools, Default::default(), config)
}

#[derive(Clone)]
pub struct Outside {
    state: Arc<State>,
    tool_router: ToolRouter<Self>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddSkill {
    /// The skill's folder, a full path: its name is the skill's, and it holds a SKILL.md with `name` and
    /// `description` in its front matter.
    folder: String,
    /// True to replace the skill of that name.
    replace: Option<bool>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddTool {
    /// A tool on the web: its link.
    link: Option<String>,
    /// A tool on this PC: the command line or the JSON block from its instructions. Give a secret's value as "" for
    /// the person to fill in.
    command: Option<String>,
    /// Its name, if not the one it gives itself.
    name: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddAgent {
    name: String,
    /// What it is for: one line, at most 80 characters.
    description: String,
    /// One of the person's accounts, by name. Needed when they have more than one.
    account: Option<String>,
    /// A model that account lists. Its engine's default if not given.
    model: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddProject {
    name: String,
    /// The project's folder, a full path in a git repository.
    folder: String,
    /// 1 to 5 of the person's agents, by name.
    team: Vec<String>,
    /// The one of them who leads.
    lead: String,
}

/// The app calling, by the name it gave as it connected.
fn app_of(parts: &Parts) -> String {
    let given = parts
        .headers
        .get(APP)
        .map(|v| String::from_utf8_lossy(v.as_bytes()));
    let name: String = given
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match name.chars().take(80).collect::<String>() {
        name if name.is_empty() => "An AI app".into(),
        name => name,
    }
}

#[tool_router]
impl Outside {
    #[tool(
        description = "What the person has in Gathr: their agents, accounts and the models each lists, projects, tools \
                       and skills. Read it before adding one."
    )]
    async fn look(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        let (started, trace) = (Instant::now(), gathr::ids::trace_id());
        let seen = look(&self.state, &trace).await;
        Ok(self.respond(&parts, "look", (&trace, started), seen).await)
    }

    #[tool(
        description = "Add a skill for every agent in Gathr from a folder you wrote on this PC (a SKILL.md with name \
                       and description in its front matter, and any other files)."
    )]
    async fn add_skill(
        &self,
        Parameters(ask): Parameters<AddSkill>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let (started, trace) = (Instant::now(), gathr::ids::trace_id());
        let (s, app) = (&self.state, app_of(&parts));
        let folder = Path::new(ask.folder.trim());
        let real = std::fs::canonicalize(folder).ok().filter(|p| p.is_dir());
        let added = match real.filter(|_| folder.is_absolute()) {
            None => Err(format!(
                "{} isn't a folder: give its full path.",
                ask.folder
            )),
            Some(real) => add::skill(s, real, ask.replace == Some(true)).await,
        };
        let added = match added {
            Ok((answer, name)) => noted(s, ("skill", name), &app).await.map(|()| said(answer)),
            Err(why) => Err(why),
        };
        Ok(self
            .respond(&parts, "add_skill", (&trace, started), added)
            .await)
    }

    #[tool(
        description = "Add a tool (an MCP server) for every agent in Gathr: its link, or the command from its \
                       instructions. Leave a secret's value empty: the person fills it in, in Gathr."
    )]
    async fn add_tool(
        &self,
        Parameters(ask): Parameters<AddTool>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let (started, trace) = (Instant::now(), gathr::ids::trace_id());
        let (s, app) = (&self.state, app_of(&parts));
        let added = add::tool(s, &trace, (ask.link, ask.command), ask.name).await;
        let added = match added {
            Ok((answer, id)) => noted(s, ("tool", id), &app).await.map(|()| said(answer)),
            Err(why) => Err(why),
        };
        Ok(self
            .respond(&parts, "add_tool", (&trace, started), added)
            .await)
    }

    #[tool(
        description = "Add an agent to the person's Gathr: its name, what it is for, and the account and model it \
                       runs on."
    )]
    async fn add_agent(
        &self,
        Parameters(ask): Parameters<AddAgent>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let (started, trace) = (Instant::now(), gathr::ids::trace_id());
        let added = agent(&self.state, &trace, ask, &app_of(&parts)).await;
        Ok(self
            .respond(&parts, "add_agent", (&trace, started), added)
            .await)
    }

    #[tool(
        description = "Make a project in the person's Gathr: its name, its git folder, and its team of their agents \
                       with the one who leads."
    )]
    async fn add_project(
        &self,
        Parameters(ask): Parameters<AddProject>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let (started, trace) = (Instant::now(), gathr::ids::trace_id());
        let made = project(&self.state, &trace, ask, &app_of(&parts)).await;
        Ok(self
            .respond(&parts, "add_project", (&trace, started), made)
            .await)
    }
}

impl Outside {
    /// A tool's answer, with its call recorded as an agent's is, by the app's name; a failure says why and gives the
    /// reference that finds it.
    async fn respond(
        &self,
        parts: &Parts,
        tool: &str,
        (trace, started): (&str, Instant),
        result: Result<String, String>,
    ) -> CallToolResult {
        let mut event = Event::point("gathr.tool.call")
            .trace(trace)
            .attr("gen_ai.tool.name", tool)
            .attr("gathr.app", app_of(parts));
        event.duration_us = Some(started.elapsed().as_micros() as i64);
        let writer = &self.state.writer;
        match result {
            Ok(text) => {
                writer.record(
                    event
                        .ok()
                        .attr("gathr.tool.result.chars", text.chars().count()),
                );
                CallToolResult::success(vec![ContentBlock::text(text)])
            }
            Err(why) => {
                writer
                    .record_now(event.error(tool).attr("why", why.clone()))
                    .await;
                let said = format!(
                    "{tool} failed: {why} (ref {})",
                    gathr::ids::reference(trace)
                );
                CallToolResult::error(vec![ContentBlock::text(said)])
            }
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Outside {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
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

/// An agent's answer as an outside app reads it: without the words about the agent's own turn.
fn said(answer: String) -> String {
    answer.replace(add::NEXT_START, add::NEXT_TURN)
}

/// Notes the app that added a skill or a tool, and tells every window again, now that it says so.
async fn noted(s: &State, (kind, id): (&'static str, String), app: &str) -> Result<(), String> {
    let app = app.to_string();
    db(s, move |c| gathr::added_by::note(c, kind, &id, &app)).await?;
    let _ = s.chats.live.send(json!({ "type": format!("{kind}s") }));
    Ok(())
}

/// What there is: never a key, a variable's value, a sign-in or a token.
async fn look(s: &Arc<State>, trace: &str) -> Result<String, String> {
    let data = s.data.path().to_path_buf();
    let (agents, accounts, projects, tools, skills) = db(s, move |c| {
        let dir = gathr::skills::folder(c, &data)?;
        let skills = gathr::skills::list(&dir, &gathr::skills::off(c)?);
        let (agents, accounts) = (gathr::agents::list(c)?, gathr::accounts::list(c)?);
        Ok((
            agents,
            accounts,
            gathr::projects::list(c)?,
            gathr::web_tools::list(c)?,
            skills,
        ))
    })
    .await?;
    let account_name = |id: &str| accounts.iter().find(|a| a.id == id).map(|a| a.name.clone());
    let agents: Vec<Value> = (agents.iter())
        .map(|a| json!({ "name": a.name, "description": a.description, "account": account_name(&a.account),
                         "model": a.model }))
        .collect();
    let mut listed = Vec::new();
    for account in &accounts {
        let models = match crate::models::list(s, trace, account).await {
            Ok(list) => json!(
                list["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|m| json!({ "id": m["id"], "name": m["name"] }))
                    .collect::<Vec<_>>()
            ),
            Err(why) => json!(format!("not listed: {why}")),
        };
        listed.push(json!({ "name": account.name, "engine": account.engine, "models": models }));
    }
    let projects: Vec<Value> = (projects.iter())
        .map(|p| {
            let lead = p
                .members
                .iter()
                .find(|m| m.role == Role::Supervisor)
                .map(|m| &m.name);
            let team: Vec<&str> = p.members.iter().map(|m| m.name.as_str()).collect();
            json!({ "name": p.name, "folder": p.folder, "team": team, "lead": lead })
        })
        .collect();
    let tools: Vec<Value> = (tools.iter())
        .map(|t| json!({ "name": t.name, "where": if t.kind == "pc" { "this PC" } else { "web" }, "enabled": t.enabled }))
        .collect();
    let skills: Vec<Value> = (skills.iter())
        .map(|k| json!({ "name": k.name, "description": k.description, "enabled": k.enabled }))
        .collect();
    let seen = json!({ "agents": agents, "accounts": listed, "projects": projects, "tools": tools, "skills": skills });
    serde_json::to_string_pretty(&seen).map_err(|e| e.to_string())
}

/// Adds the agent asked for, checked as `propose_agent` checks one, as the Agents screen adds one.
async fn agent(s: &Arc<State>, trace: &str, ask: AddAgent, app: &str) -> Result<String, String> {
    let asked = (
        ask.name.clone(),
        ask.description.clone(),
        ask.account.clone(),
    );
    let (mut new, account) = db(s, move |c| {
        drafted(c, (&asked.0, &asked.1, asked.2.as_deref()), None)
    })
    .await?;
    if let Some(model) = &ask.model {
        let listed = crate::models::list(s, trace, &account)
            .await
            .map_err(|why| format!("The models of {} weren't listed: {why}", account.name))?;
        new.model = Some(listed_model(&listed, model, &account)?);
    }
    let (chats, app) = (s.chats.clone(), app.to_string());
    let added = db(s, move |c| {
        let added = gathr::agents::add(c, &new.agent(), &chats.being_removed())?;
        if let Ok(agent) = &added {
            gathr::added_by::note(c, "agent", &agent.id, &app)?;
        }
        Ok(added)
    })
    .await?;
    let agent = added.map_err(|why| format!("{}.", why.why().trim_end_matches('.')))?;
    crate::agents::record(s, &Trace(trace.to_string()), "add", &agent, None);
    Ok(format!("Added the agent:\n{}", agent.name))
}

/// Makes the project asked for, checked as `propose_project` checks one, as Settings makes one: no first chat.
async fn project(
    s: &Arc<State>,
    trace: &str,
    ask: AddProject,
    app: &str,
) -> Result<String, String> {
    let folder = crate::team::top_level(Path::new(ask.folder.trim()))
        .await
        .map_err(|why| format!("{why}: give the full path of a git folder."))?;
    let app = app.to_string();
    let made = db(s, move |c| {
        let name = project_name(c, &ask.name)?;
        let role = |lead| if lead { Role::Supervisor } else { Role::Member };
        let seats: Vec<(String, Role)> = (team(c, &ask.team, &ask.lead)?.into_iter())
            .map(|seat| (seat.agent, role(seat.lead)))
            .collect();
        let made = gathr::projects::create(c, &name, &folder, &seats)?;
        gathr::added_by::note(c, "project", &made.id, &app)?;
        Ok(made)
    })
    .await?;
    let event = Event::point("gathr.project.create").trace(trace).ok();
    s.writer
        .record(event.attr("gathr.project.id", made.id.clone()));
    s.chats
        .publish(json!({ "type": "project", "project": made.id, "change": "created" }));
    Ok(format!("Made the project:\n{}", made.name))
}
