//! `show`: an agent puts a table, a chart, a flow, math steps, a diagram or a page in the chat, under
//! its words. A step keeps only a line of a tool's input, so the block is a message of its own in the
//! caller's turn, of kind `block` (0078), its body `{kind, title?, <the kind's field>}`: in the chat's
//! history, live in an open window, never searched. A diagram's picture or a page is a file the
//! window already shows from the chat's folders: anything else is refused.

use std::collections::HashMap;
use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::server::{Tools, caller_of};

const MOST_BYTES: usize = 100_000;
const TABLE: &str = "table is {columns: [{key, label, as?}], rows: [{<key>: value}]}";
const CHART: &str =
    "chart is {label, value, points: [numbers], labels?: [a name per point], change?, vs?, style?}";
const FLOW: &str = "flow is {nodes: [{id, label, after?: [ids], state?}]}";
const MATH: &str = "math is {label?, steps: [{expression, note?}]}";
const DIAGRAM: &str =
    "diagram is {mermaid} or {file}: a png, jpg, gif, webp or svg in the chat's folders";
const PAGE: &str = "page is {file: an .html in the chat's folders, height?: 160 to 720}";
const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Table,
    Chart,
    Flow,
    Math,
    Diagram,
    Page,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Show {
    /// Give the field of that name.
    kind: Kind,
    /// Drawn small above it. At most 80 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<Table>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chart: Option<Chart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<Flow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    math: Option<Math>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagram: Option<Diagram>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<Page>,
}

/// One of the two.
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Diagram {
    /// Mermaid source, at most 20,000 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    mermaid: Option<String>,
    /// A picture's full path: png, jpg, gif, webp or svg.
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Page {
    /// An .html file's full path; it runs sealed.
    file: String,
    /// 160 to 720 pixels; 360 unless set.
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u16>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Table {
    /// 1 to 12.
    columns: Vec<Column>,
    /// Up to 200, keyed by column key. A value is a string, number, boolean or null.
    rows: Vec<Map<String, Value>>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Column {
    key: String,
    label: String,
    /// text unless set. percent: 12.5 is 12.5%. change: a signed number. date: ISO. link: an http(s) URL or [name](URL).
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    shown_as: Option<As>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    /// ISO code, for money.
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<String>,
    /// 0 to 6.
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
    /// For change: true unless set.
    #[serde(skip_serializing_if = "Option::is_none")]
    up_is_good: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum As {
    Text,
    Number,
    Money,
    Percent,
    Change,
    Date,
    Link,
    YesNo,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Chart {
    /// At most 60 characters.
    label: String,
    /// The headline, at most 24 characters, saying which point it is when it is one: "1,240 in Nov".
    value: String,
    /// 2 to 500.
    points: Vec<f64>,
    /// A name for each point, in order, at most 12 characters each: "Jan", "Feb". The first and last show under
    /// the chart, and the high point's beside it.
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    /// At most 16 characters: "+3.7".
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<String>,
    /// What `change` is measured against, at most 16 characters: "Oct", "last year". Shown as "vs Oct".
    #[serde(skip_serializing_if = "Option::is_none")]
    vs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<Style>,
    #[serde(skip_serializing_if = "Option::is_none")]
    up_is_good: Option<bool>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Line,
    Area,
    Bars,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Flow {
    /// 2 to 40.
    nodes: Vec<Node>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Node {
    id: String,
    /// At most 60 characters.
    label: String,
    /// Ids it follows.
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<Vec<String>>,
    /// pending unless set.
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<State>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Done,
    Active,
    Pending,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Math {
    /// At most 60 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// 1 to 30.
    steps: Vec<Step>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Step {
    /// Plain text, not LaTeX: x^2, a_{ij}. At most 200 characters.
    expression: String,
    /// At most 120 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[tool_router(router = show_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Show the person a table, a chart, a flow of work, math steps, a diagram or a page you made \
                       in the chat, under your words, when it reads better drawn than written."
    )]
    async fn show(
        &self,
        Parameters(ask): Parameters<Show>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, data) = (caller.chat.clone(), self.chats.data.path().to_path_buf());
        let shown = match (block(ask), caller.on()) {
            (Err(why), _) => Err(why),
            (_, None) => Err("you have no turn running in this chat".into()),
            (Ok((body, file)), Some(on)) => self
                .chats
                .writer
                .call(move |c| {
                    gathr::archive::refuse_archived(c, &chat)?;
                    // Only a file the window already shows from the chat's folders.
                    if let Some(file) = &file {
                        let roots = gathr::files::tree(c, &data, &chat)?.roots();
                        gathr::files::attached(&roots, std::slice::from_ref(file))?;
                    }
                    gathr::store::add_message(c, &chat, &on.turn, "assistant", "block", &body)
                })
                .await
                .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
                .map_err(|e| e.to_string()),
        };
        let said = shown.map(|message| {
            let live = json!({ "type": "message", "chat": caller.chat, "message": message });
            self.chats.publish(live);
            "Shown.".to_string()
        });
        Ok(self.respond(&caller, "show", started, said).await)
    }
}

/// The block's body as stored with the file it shows, if any, or what is wrong with it in a line or two.
fn block(mut ask: Show) -> Result<(String, Option<String>), String> {
    if ask.title.as_ref().is_some_and(|t| t.chars().count() > 80) {
        return Err("title is at most 80 characters.".into());
    }
    // Only the kind's own field is kept.
    let (table, chart) = (ask.table.take(), ask.chart.take());
    let (flow, math) = (ask.flow.take(), ask.math.take());
    let (diagram, page) = (ask.diagram.take(), ask.page.take());
    let mut file = None;
    match ask.kind {
        Kind::Table => {
            let table = table.ok_or(format!("kind table needs table: {TABLE}."))?;
            check_table(&table).map_err(|why| format!("{why}\nA {TABLE}."))?;
            ask.table = Some(table);
        }
        Kind::Chart => {
            let chart = chart.ok_or(format!("kind chart needs chart: {CHART}."))?;
            check_chart(&chart).map_err(|why| format!("{why}\nA {CHART}."))?;
            ask.chart = Some(chart);
        }
        Kind::Flow => {
            let flow = flow.ok_or(format!("kind flow needs flow: {FLOW}."))?;
            check_flow(&flow).map_err(|why| format!("{why}\nA {FLOW}."))?;
            ask.flow = Some(flow);
        }
        Kind::Math => {
            let math = math.ok_or(format!("kind math needs math: {MATH}."))?;
            check_math(&math).map_err(|why| format!("{why}\nA {MATH}."))?;
            ask.math = Some(math);
        }
        Kind::Diagram => {
            let diagram = diagram.ok_or(format!("kind diagram needs diagram: {DIAGRAM}."))?;
            file = check_diagram(&diagram).map_err(|why| format!("{why}\nA {DIAGRAM}."))?;
            ask.diagram = Some(diagram);
        }
        Kind::Page => {
            let page = page.ok_or(format!("kind page needs page: {PAGE}."))?;
            check_page(&page).map_err(|why| format!("{why}\nA {PAGE}."))?;
            file = Some(page.file.clone());
            ask.page = Some(page);
        }
    }
    let body = serde_json::to_string(&ask).map_err(|e| e.to_string())?;
    if body.len() > MOST_BYTES {
        return Err(format!("It is {} KB; at most 100 KB.", body.len() / 1000));
    }
    Ok((body, file))
}

/// `path`'s extension, lowercased.
fn extension(path: &str) -> String {
    let ext = std::path::Path::new(path).extension().unwrap_or_default();
    ext.to_string_lossy().to_lowercase()
}

/// The picture file it shows, if it shows one.
fn check_diagram(diagram: &Diagram) -> Result<Option<String>, String> {
    match (&diagram.mermaid, &diagram.file) {
        (Some(source), None) => {
            let n = source.chars().count();
            if source.trim().is_empty() || n > 20_000 {
                return Err(format!("mermaid is {n} characters: give 1 to 20,000."));
            }
            Ok(None)
        }
        (None, Some(file)) if PICTURES.contains(&extension(file).as_str()) => {
            Ok(Some(file.clone()))
        }
        (None, Some(file)) => Err(format!("{file} isn't a png, jpg, gif, webp or svg.")),
        _ => Err("Give one of mermaid and file.".into()),
    }
}

fn check_page(page: &Page) -> Result<(), String> {
    if !matches!(extension(&page.file).as_str(), "html" | "htm") {
        return Err(format!("{} isn't an .html file.", page.file));
    }
    if page.height.is_some_and(|h| !(160..=720).contains(&h)) {
        return Err("height is 160 to 720.".into());
    }
    Ok(())
}

fn check_table(table: &Table) -> Result<(), String> {
    let n = table.columns.len();
    if !(1..=12).contains(&n) {
        return Err(format!("{n} columns: give 1 to 12."));
    }
    if table.rows.len() > 200 {
        return Err(format!("{} rows: give at most 200.", table.rows.len()));
    }
    for (i, column) in table.columns.iter().enumerate() {
        let key = &column.key;
        if key.is_empty() || table.columns[..i].iter().any(|c| c.key == *key) {
            return Err(format!("Column key \"{key}\" is empty or taken twice."));
        }
        if column.decimals.is_some_and(|d| d > 6) {
            return Err(format!("Column {key}: decimals is 0 to 6."));
        }
        let code = column.currency.as_deref().unwrap_or_default();
        let iso = code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase());
        if column.shown_as == Some(As::Money) && !iso {
            return Err(format!(
                "Column {key}: money needs currency, an ISO code such as USD."
            ));
        }
    }
    for (i, row) in table.rows.iter().enumerate() {
        for column in &table.columns {
            let value = row.get(&column.key).unwrap_or(&Value::Null);
            let shown_as = column.shown_as.unwrap_or(As::Text);
            if !fits(value, shown_as) {
                let key = &column.key;
                return Err(format!(
                    "Row {i}, {key}: give {} or null.",
                    wanted(shown_as)
                ));
            }
        }
        let long = row
            .values()
            .any(|v| v.as_str().is_some_and(|s| s.chars().count() > 500));
        if long || row.values().any(|v| v.is_array() || v.is_object()) {
            return Err(format!(
                "Row {i}: a value is a string of at most 500 characters, a number, a boolean or null."
            ));
        }
    }
    Ok(())
}

fn fits(value: &Value, shown_as: As) -> bool {
    match (value, shown_as) {
        (Value::Null, _) => true,
        (Value::Number(_), As::Number | As::Money | As::Percent | As::Change) => true,
        (Value::Bool(_), As::YesNo) => true,
        (Value::String(s), As::Date) => is_date(s),
        (Value::String(s), As::Link) => {
            let address = named_link(s).map_or(s.as_str(), |(_, address)| address);
            address.starts_with("https://") || address.starts_with("http://")
        }
        (Value::String(_) | Value::Number(_) | Value::Bool(_), As::Text) => true,
        _ => false,
    }
}

fn wanted(shown_as: As) -> &'static str {
    match shown_as {
        As::Number | As::Money | As::Percent | As::Change => "a number",
        As::YesNo => "true or false",
        As::Date => "an ISO date, 2026-11-22",
        As::Link => "an http(s) URL or [name](URL)",
        As::Text => "a string, number or boolean",
    }
}

/// `[Seed shop](https://seeds.example)` as its name and its address.
fn named_link(s: &str) -> Option<(&str, &str)> {
    s.strip_prefix('[')?.strip_suffix(')')?.rsplit_once("](")
}

/// `2026-11-22`, alone or followed by a time.
fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    digits(0..4)
        && b.get(4) == Some(&b'-')
        && digits(5..7)
        && b.get(7) == Some(&b'-')
        && digits(8..10)
        && (b.len() == 10 || b[10] == b'T' || b[10] == b' ')
}

fn check_chart(chart: &Chart) -> Result<(), String> {
    let long = |text: &str, most: usize| text.chars().count() > most;
    if long(&chart.label, 60)
        || long(&chart.value, 24)
        || chart.change.as_deref().is_some_and(|c| long(c, 16))
        || chart.vs.as_deref().is_some_and(|v| long(v, 16))
    {
        return Err("label is at most 60 characters, value 24, change and vs 16.".into());
    }
    let n = chart.points.len();
    if !(2..=500).contains(&n) {
        return Err(format!("{n} points: give 2 to 500."));
    }
    if let Some(labels) = &chart.labels {
        if labels.len() != n {
            return Err(format!(
                "{} labels for {n} points: give one for each point.",
                labels.len()
            ));
        }
        if labels.iter().any(|l| long(l, 12)) {
            return Err("Each label is at most 12 characters.".into());
        }
    }
    Ok(())
}

fn check_flow(flow: &Flow) -> Result<(), String> {
    let nodes = &flow.nodes;
    if !(2..=40).contains(&nodes.len()) {
        return Err(format!("{} nodes: give 2 to 40.", nodes.len()));
    }
    let mut at = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.id.is_empty() || at.insert(node.id.as_str(), i).is_some() {
            return Err(format!("Node id \"{}\" is empty or taken twice.", node.id));
        }
        if node.label.chars().count() > 60 {
            return Err(format!("Node {}: label is at most 60 characters.", node.id));
        }
    }
    let mut after = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut before = Vec::new();
        for id in node.after.iter().flatten() {
            let i = at.get(id.as_str()).ok_or(format!(
                "Node {}: after names {id}, which is not a node.",
                node.id
            ))?;
            before.push(*i);
        }
        after.push(before);
    }
    match looped(&after) {
        Some(lap) => {
            let ids: Vec<&str> = lap.iter().map(|&i| nodes[i].id.as_str()).collect();
            Err(format!(
                "after goes round in a loop: {}. A node comes after nodes that come before it.",
                ids.join(" -> ")
            ))
        }
        None => Ok(()),
    }
}

/// A loop in `after` (each node's indexes of the nodes it follows), as the indexes round it, first
/// one again at the end; none when every node can be placed after those it follows.
fn looped(after: &[Vec<usize>]) -> Option<Vec<usize>> {
    // 0: not reached, 1: on the path being walked, 2: placed.
    fn walk(
        i: usize,
        after: &[Vec<usize>],
        mark: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        mark[i] = 1;
        path.push(i);
        for &j in &after[i] {
            if mark[j] == 1 {
                let from = path.iter().position(|&p| p == j).unwrap_or(0);
                let mut lap: Vec<usize> = path[from..].iter().rev().copied().collect();
                lap.push(lap[0]);
                return Some(lap);
            }
            if mark[j] == 0
                && let Some(lap) = walk(j, after, mark, path)
            {
                return Some(lap);
            }
        }
        mark[i] = 2;
        path.pop();
        None
    }
    let mut mark = vec![0; after.len()];
    (0..after.len()).find_map(|i| {
        if mark[i] != 0 {
            return None;
        }
        walk(i, after, &mut mark, &mut Vec::new())
    })
}

fn check_math(math: &Math) -> Result<(), String> {
    let long = |text: &str, most: usize| text.chars().count() > most;
    if math.label.as_deref().is_some_and(|l| long(l, 60)) {
        return Err("label is at most 60 characters.".into());
    }
    let n = math.steps.len();
    if !(1..=30).contains(&n) {
        return Err(format!("{n} steps: give 1 to 30."));
    }
    for (i, step) in math.steps.iter().enumerate() {
        if long(&step.expression, 200) || step.note.as_deref().is_some_and(|n| long(n, 120)) {
            return Err(format!(
                "Step {i}: expression is at most 200 characters, note 120."
            ));
        }
    }
    Ok(())
}

/// A block as `read_chat` gives it: "[table: Beds, 12 rows]", "[chart: Rain, 30 points]",
/// "[flow: Planting, 7 steps]", "[math: Compost, 4 steps]", "[diagram: Beds]", "[page: beds.html]"; a proposed project
/// (`propose_project`) "[project: Garden app, proposed]" or "[project: Garden app, made]", a proposed agent
/// (`propose_agent`) "[agent: Kit, proposed]" or "[agent: Kit, added]", a proposed change (`propose_agent_change`,
/// `propose_project_change`) "[agent change: Kit, proposed]" or "[project change: Garden app, saved]".
pub(super) fn line(body: &str) -> String {
    let block: Value = serde_json::from_str(body).unwrap_or_default();
    let kind = block["kind"].as_str().unwrap_or("block");
    if kind == "agent_change" || kind == "project_change" {
        let change = &block[kind];
        let state = if change["saved"] == true {
            "saved"
        } else {
            "proposed"
        };
        let name = change["was"]["name"].as_str().unwrap_or_default();
        return format!("[{}: {name}, {state}]", kind.replace('_', " "));
    }
    if kind == "project" || kind == "agent" {
        let proposed = &block[kind];
        let done = if kind == "project" { "made" } else { "added" };
        let state = if proposed[done].is_null() {
            "proposed"
        } else {
            done
        };
        let name = proposed["name"].as_str().unwrap_or_default();
        return format!("[{kind}: {name}, {state}]");
    }
    if kind == "diagram" || kind == "page" {
        let file = block["page"]["file"].as_str().unwrap_or_default();
        let name = file.rsplit(['/', '\\']).next().unwrap_or_default();
        return match block["title"].as_str().filter(|t| !t.is_empty()) {
            Some(title) => format!("[{kind}: {title}]"),
            None if kind == "page" => format!("[page: {name}]"),
            None => "[diagram]".into(),
        };
    }
    let count = |list: &Value| list.as_array().map_or(0, Vec::len);
    let (count, noun) = match kind {
        "table" => (count(&block["table"]["rows"]), "row"),
        "flow" => (count(&block["flow"]["nodes"]), "step"),
        "math" => (count(&block["math"]["steps"]), "step"),
        _ => (count(&block["chart"]["points"]), "point"),
    };
    let named = block["title"]
        .as_str()
        .or(block["chart"]["label"].as_str())
        .or(block["math"]["label"].as_str());
    let plural = if count == 1 { "" } else { "s" };
    match named {
        Some(name) => format!("[{kind}: {name}, {count} {noun}{plural}]"),
        None => format!("[{kind}, {count} {noun}{plural}]"),
    }
}
