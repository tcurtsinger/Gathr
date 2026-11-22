//! `show`: an agent puts a table or a chart in the chat, under its words. A step keeps only a line of
//! a tool's input, so the block is a message of its own in the caller's turn, of kind `block` (0078),
//! its body `{kind, title?, table | chart}`: in the chat's history, live in an open window, never searched.

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
const CHART: &str = "chart is {label, value, points: [numbers], change?, style?}";

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Table,
    Chart,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Show {
    kind: Kind,
    /// Drawn small above it. At most 80 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// For kind table.
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<Table>,
    /// For kind chart.
    #[serde(skip_serializing_if = "Option::is_none")]
    chart: Option<Chart>,
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
    /// text unless set. percent: 12.5 is 12.5%. change: a signed number. date: ISO. link: an http(s) URL.
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
    /// The headline, at most 24 characters: "16.8 pts".
    value: String,
    /// 2 to 500.
    points: Vec<f64>,
    /// At most 16 characters: "+3.7".
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<String>,
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

#[tool_router(router = show_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Show the person a table or a chart in the chat, under your words: rows and columns, or a series \
                       of numbers, that read better drawn than written."
    )]
    async fn show(
        &self,
        Parameters(ask): Parameters<Show>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let chat = caller.chat.clone();
        let shown = match (block(ask), caller.on()) {
            (Err(why), _) => Err(why),
            (_, None) => Err("you have no turn running in this chat".into()),
            (Ok(body), Some(on)) => self
                .chats
                .writer
                .call(move |c| {
                    gathr::archive::refuse_archived(c, &chat)?;
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

/// The block's body as stored, or what is wrong with it in a line or two.
fn block(mut ask: Show) -> Result<String, String> {
    if ask.title.as_ref().is_some_and(|t| t.chars().count() > 80) {
        return Err("title is at most 80 characters.".into());
    }
    match ask.kind {
        Kind::Table => {
            ask.chart = None;
            let table = ask
                .table
                .as_ref()
                .ok_or(format!("kind table needs table: {TABLE}."))?;
            check_table(table).map_err(|why| format!("{why}\nA {TABLE}."))?;
        }
        Kind::Chart => {
            ask.table = None;
            let chart = ask
                .chart
                .as_ref()
                .ok_or(format!("kind chart needs chart: {CHART}."))?;
            check_chart(chart).map_err(|why| format!("{why}\nA {CHART}."))?;
        }
    }
    let body = serde_json::to_string(&ask).map_err(|e| e.to_string())?;
    if body.len() > MOST_BYTES {
        return Err(format!("It is {} KB; at most 100 KB.", body.len() / 1000));
    }
    Ok(body)
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
        (Value::String(s), As::Link) => s.starts_with("https://") || s.starts_with("http://"),
        (Value::String(_) | Value::Number(_) | Value::Bool(_), As::Text) => true,
        _ => false,
    }
}

fn wanted(shown_as: As) -> &'static str {
    match shown_as {
        As::Number | As::Money | As::Percent | As::Change => "a number",
        As::YesNo => "true or false",
        As::Date => "an ISO date, 2026-11-22",
        As::Link => "an http(s) URL",
        As::Text => "a string, number or boolean",
    }
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
    {
        return Err("label is at most 60 characters, value 24, change 16.".into());
    }
    let n = chart.points.len();
    if !(2..=500).contains(&n) {
        return Err(format!("{n} points: give 2 to 500."));
    }
    Ok(())
}

/// A block as `read_chat` gives it: "[table: Beds, 12 rows]", "[chart: Rain, 30 points]".
pub(super) fn line(body: &str) -> String {
    let block: Value = serde_json::from_str(body).unwrap_or_default();
    let kind = block["kind"].as_str().unwrap_or("block");
    let (count, noun) = match kind {
        "table" => (block["table"]["rows"].as_array().map_or(0, Vec::len), "row"),
        _ => (
            block["chart"]["points"].as_array().map_or(0, Vec::len),
            "point",
        ),
    };
    let named = block["title"].as_str().or(block["chart"]["label"].as_str());
    let plural = if count == 1 { "" } else { "s" };
    match named {
        Some(name) => format!("[{kind}: {name}, {count} {noun}{plural}]"),
        None => format!("[{kind}, {count} {noun}{plural}]"),
    }
}
