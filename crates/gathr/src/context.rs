//! How full each agent's context is in each chat (compaction plan, K1): the prompt of its last model
//! call and its model's window, as the engine reports them, kept across restarts.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::now_us;

/// The agent's last call filled this much: its context as now shown.
pub fn filled(conn: &Connection, chat: &str, agent: &str, used: i64) -> Result<Value, Error> {
    conn.execute(
        "INSERT INTO agent_context (chat_id, agent_id, used, updated_ts) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (chat_id, agent_id) DO UPDATE SET used = ?3, updated_ts = ?4",
        params![chat, agent, used, now_us()],
    )?;
    Ok(of(conn, chat, agent)?.unwrap_or(Value::Null))
}

/// The agent's model holds this much, seen while the agent was set to `set` (`model_key`) and its
/// calls ran on `ran`, the model the engine named: its context as now shown.
pub fn window(
    conn: &Connection,
    chat: &str,
    agent: &str,
    window: i64,
    set: &str,
    ran: Option<&str>,
) -> Result<Value, Error> {
    conn.execute(
        "INSERT INTO agent_context (chat_id, agent_id, window, window_of, ran, updated_ts)
         VALUES (?1, ?2, ?3, ?5, ?6, ?4)
         ON CONFLICT (chat_id, agent_id) DO UPDATE SET window = ?3, window_of = ?5, ran = ?6, updated_ts = ?4",
        params![chat, agent, window, now_us(), set, ran],
    )?;
    Ok(of(conn, chat, agent)?.unwrap_or(Value::Null))
}

/// `{used, window, compact_at}`, each null until known; none before the agent's first call.
/// `compact_at` is what the agent's engine starts with: none unless the window was seen on the
/// agent's account and model as they are set now, on a model the engine named.
pub fn of(conn: &Connection, chat: &str, agent: &str) -> Result<Option<Value>, Error> {
    type Row = (Option<i64>, Option<i64>, Option<String>, Option<String>);
    let found: Option<Row> = conn
        .query_row(
            "SELECT used, window, window_of, ran FROM agent_context WHERE chat_id = ?1 AND agent_id = ?2",
            params![chat, agent],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((used, window, set, ran)) = found else {
        return Ok(None);
    };
    let now = agent_key(conn, agent)?;
    let at = match (window, set, ran) {
        (Some(window), Some(set), Some(_)) if now.as_deref() == Some(set.as_str()) => {
            point(auto_compact(conn)?, Some(window))
        }
        _ => None,
    };
    Ok(Some(
        json!({ "used": used, "window": window, "compact_at": at }),
    ))
}

/// The agent's `model_key` as it's set now.
fn agent_key(conn: &Connection, agent: &str) -> Result<Option<String>, Error> {
    let Some(agent) = crate::agents::get(conn, agent)? else {
        return Ok(None);
    };
    let Some(account) = crate::accounts::get(conn, &agent.account)? else {
        return Ok(None);
    };
    Ok(Some(model_key(
        &account.id,
        &account.engine,
        agent.model.as_deref(),
    )))
}

const SETTING: &str = "auto_compact";
const ENGINES_OWN: &str = "engine";
/// Auto-compact's choices in Settings > General (compaction plan, K3), 500K unless set.
pub const AUTO_COMPACT: [i64; 4] = [200_000, 300_000, 400_000, 500_000];
const DEFAULT: i64 = 500_000;

/// Where agents' conversations compact: one of `AUTO_COMPACT`, or `None` for each engine's own.
pub fn auto_compact(conn: &Connection) -> Result<Option<i64>, Error> {
    let saved: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match saved.as_deref() {
        None => Some(DEFAULT),
        Some(ENGINES_OWN) => None,
        Some(n) => Some(n.parse().unwrap_or(DEFAULT)),
    })
}

/// Saves Auto-compact: one of `AUTO_COMPACT`, or `None` for each engine's own.
pub fn set_auto_compact(conn: &Connection, at: Option<i64>) -> Result<(), Error> {
    if at.is_some_and(|n| !AUTO_COMPACT.contains(&n)) {
        return Err(Error::Refused(
            "Auto-compact is 200000, 300000, 400000 or 500000 tokens, or null for each engine's own".into(),
        ));
    }
    let value = at.map_or(ENGINES_OWN.to_string(), |n| n.to_string());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![SETTING, value],
    )?;
    Ok(())
}

/// Where a conversation compacts: the setting, never past 80% of the model's window, so a small
/// window compacts sooner. `None` leaves it to the engine.
pub fn point(setting: Option<i64>, window: Option<i64>) -> Option<i64> {
    setting.map(|at| window.map_or(at, |w| at.min(w * 8 / 10)))
}

/// What an agent is set to: its account, engine and model, `default` for its engine's own. A
/// pinned model always runs itself. The engine's default and a Latest (Claude Code's `opus`,
/// Gathr's `latest:<family>`) can be another model at a later start, so each window is kept with
/// the model the engine named (`ran`), and a point is only ever given for that model.
pub fn model_key(account: &str, engine: &str, model: Option<&str>) -> String {
    format!("{account}:{engine}:{}", model.unwrap_or("default"))
}

/// The point for the agent's engine about to start in `chat`, set to `set` (`model_key`), and the
/// model it's for: from the window of the model the agent last ran while set so, in this chat or
/// else its latest anywhere. None while no window is known, so the engine compacts at its own point
/// rather than one past its window (#168's review).
pub fn point_for(
    conn: &Connection,
    chat: &str,
    agent: &str,
    set: &str,
) -> Result<Option<(i64, String)>, Error> {
    let seen: Option<(i64, String)> = conn
        .query_row(
            "SELECT window, ran FROM agent_context
             WHERE agent_id = ?2 AND window IS NOT NULL AND window_of = ?3 AND ran IS NOT NULL
             ORDER BY chat_id = ?1 DESC, updated_ts DESC LIMIT 1",
            params![chat, agent, set],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((window, ran)) = seen else {
        return Ok(None);
    };
    Ok(point(auto_compact(conn)?, Some(window)).map(|at| (at, ran)))
}

/// The agent, set to `set`, ran on `ran` when its point was for another model: a Latest or its
/// engine's default moved. Every window seen on another model while set so stops giving a point,
/// so the engine's next start has none until `ran`'s own window is seen.
pub fn moved(conn: &Connection, agent: &str, set: &str, ran: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE agent_context SET window_of = NULL WHERE agent_id = ?1 AND window_of = ?2 AND ran IS NOT ?3",
        params![agent, set, ran],
    )?;
    Ok(())
}

/// The line a turn is given once its agent's conversation nears where it compacts (K3).
pub const SOON: &str = "Your conversation is close to its compaction point: when it compacts, earlier detail goes. \
    Write down what you still need (your notes, the task, the plan) in your own folder now.";

/// Whether the agent's next turn in `chat` gets `SOON`: its last call filled 75% of where it compacts,
/// and it hasn't been told since its last compaction. Marked told when it does.
pub fn tell_soon(conn: &Connection, chat: &str, agent: &str, set: &str) -> Result<bool, Error> {
    let found: Option<(Option<i64>, bool)> = conn
        .query_row(
            "SELECT used, told FROM agent_context WHERE chat_id = ?1 AND agent_id = ?2",
            params![chat, agent],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((Some(used), false)) = found else {
        return Ok(false);
    };
    let Some((at, _)) = point_for(conn, chat, agent, set)? else {
        return Ok(false);
    };
    if used * 4 < at * 3 {
        return Ok(false);
    }
    conn.execute(
        "UPDATE agent_context SET told = 1 WHERE chat_id = ?1 AND agent_id = ?2",
        params![chat, agent],
    )?;
    Ok(true)
}

/// Told again after the conversation compacts, or a turn fails.
pub fn untold(conn: &Connection, chat: &str, agent: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE agent_context SET told = 0 WHERE chat_id = ?1 AND agent_id = ?2",
        params![chat, agent],
    )?;
    Ok(())
}

/// Each member of a chat's history with how full its context is (`context`, null before its first call).
pub fn on_members(conn: &Connection, chat: &str, history: &mut Value) -> Result<(), Error> {
    let Some(members) = history["members"].as_array_mut() else {
        return Ok(());
    };
    for member in members {
        let agent = member["agent"].as_str().unwrap_or_default().to_string();
        member["context"] = of(conn, chat, &agent)?.unwrap_or(Value::Null);
    }
    Ok(())
}

/// The quiet line a compaction leaves in the chat: "Compacted Ann's conversation: 410K to 52K", the
/// sizes when the engine gives them (Claude Code does; Codex doesn't).
pub fn compacted_line(
    conn: &Connection,
    agent: &str,
    before: Option<i64>,
    after: Option<i64>,
) -> Result<String, Error> {
    let name = crate::agents::get(conn, agent)?.map_or_else(|| "an agent".to_string(), |a| a.name);
    Ok(match (before, after) {
        (Some(before), Some(after)) => {
            format!(
                "Compacted {name}'s conversation: {} to {}",
                tokens(before),
                tokens(after)
            )
        }
        _ => format!("Compacted {name}'s conversation"),
    })
}

/// 950, 3.6K, 23K, 1.2M: tokens as the line reads them.
fn tokens(n: i64) -> String {
    let short = |v: f64, unit: &str| {
        let shown = if v < 10.0 {
            format!("{:.1}", v)
        } else {
            format!("{:.0}", v)
        };
        format!("{}{unit}", shown.trim_end_matches(".0"))
    };
    match n {
        n if n < 1_000 => n.to_string(),
        n if n < 1_000_000 => short(n as f64 / 1_000.0, "K"),
        n => short(n as f64 / 1_000_000.0, "M"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("gathr-context-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 0), ('d', 'Other', 0);
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('a1', 'claude', 'A', 'a', 0);
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('a2', 'claude', 'B', 'b', 0);
             INSERT INTO agents (id, name, name_key, account_id, model, created_ts)
                 VALUES ('ann', 'Ann', 'ann', 'a1', 'claude-opus-5', 0);",
        )
        .unwrap();
        (dir, conn)
    }

    #[test]
    fn the_point_is_the_setting_never_past_80_percent_of_the_window() {
        assert_eq!(point(Some(500_000), Some(1_000_000)), Some(500_000));
        assert_eq!(point(Some(500_000), Some(200_000)), Some(160_000));
        assert_eq!(point(Some(300_000), None), Some(300_000));
        assert_eq!(point(None, Some(200_000)), None, "the engine's own");
    }

    #[test]
    fn auto_compact_is_one_of_its_choices_or_the_engines_own() {
        let (_dir, conn) = db();
        assert_eq!(auto_compact(&conn).unwrap(), Some(500_000));
        set_auto_compact(&conn, Some(300_000)).unwrap();
        assert_eq!(auto_compact(&conn).unwrap(), Some(300_000));
        set_auto_compact(&conn, None).unwrap();
        assert_eq!(auto_compact(&conn).unwrap(), None);
        assert!(set_auto_compact(&conn, Some(250_000)).is_err());
    }

    const OPUS: &str = "a1:claude:claude-opus-5";
    const RAN: Option<&str> = Some("claude-opus-5");

    /// Every agent has a key: what it's set to, `default` for its engine's own.
    #[test]
    fn a_key_is_what_the_agent_is_set_to_on_its_account() {
        assert_eq!(model_key("a1", "claude", Some("claude-opus-5")), OPUS);
        assert_eq!(model_key("a1", "claude", None), "a1:claude:default");
        assert_eq!(model_key("a1", "claude", Some("opus")), "a1:claude:opus");
        assert_eq!(
            model_key("a1", "codex", Some("latest:sol")),
            "a1:codex:latest:sol"
        );
    }

    /// What members show is what an engine starts with: no point until a window was seen on the
    /// agent's account and model as set, on a model the engine named, and none again once either
    /// changes.
    #[test]
    fn the_point_shown_is_the_agents_own_models() {
        let (_dir, conn) = db();
        let shown = filled(&conn, "c", "ann", 1_000).unwrap();
        assert!(shown["compact_at"].is_null(), "{shown}");
        let shown = window(&conn, "c", "ann", 200_000, OPUS, None).unwrap();
        assert!(shown["compact_at"].is_null(), "no model named: {shown}");
        let shown = window(&conn, "c", "ann", 200_000, OPUS, RAN).unwrap();
        assert_eq!(shown["compact_at"], 160_000, "{shown}");
        conn.execute(
            "UPDATE agents SET model = 'claude-sonnet-5' WHERE id = 'ann'",
            [],
        )
        .unwrap();
        let shown = of(&conn, "c", "ann").unwrap().unwrap();
        assert!(shown["compact_at"].is_null(), "another model: {shown}");
        conn.execute(
            "UPDATE agents SET model = 'claude-opus-5', account_id = 'a2' WHERE id = 'ann'",
            [],
        )
        .unwrap();
        let shown = of(&conn, "c", "ann").unwrap().unwrap();
        assert!(shown["compact_at"].is_null(), "another account: {shown}");
    }

    /// The window an engine starts with: its chat's, else the agent's latest anywhere, as set.
    #[test]
    fn a_new_chat_takes_the_agents_last_window_on_the_same_model() {
        let (_dir, conn) = db();
        let at = |conn: &Connection, set: &str| point_for(conn, "c", "ann", set).unwrap();
        assert_eq!(at(&conn, OPUS), None, "none seen yet");
        window(&conn, "d", "ann", 200_000, OPUS, RAN).unwrap();
        assert_eq!(at(&conn, OPUS), Some((160_000, "claude-opus-5".into())));
        let other = model_key("a2", "claude", Some("claude-opus-5"));
        assert_eq!(at(&conn, &other), None, "another account");
        assert_eq!(at(&conn, "a1:claude:opus"), None, "set to something else");
        window(&conn, "c", "ann", 1_000_000, OPUS, RAN).unwrap();
        assert_eq!(at(&conn, OPUS), Some((500_000, "claude-opus-5".into())));
    }

    /// An agent on a Latest or its engine's default gets its point from the model it last ran, and
    /// never keeps it once that model is another: a smaller window can't inherit a larger point.
    #[test]
    fn a_latest_takes_the_window_of_the_model_it_ran_and_drops_it_when_that_moves() {
        let (_dir, conn) = db();
        let latest = model_key("a1", "claude", Some("opus"));
        window(&conn, "d", "ann", 1_000_000, &latest, Some("claude-opus-5")).unwrap();
        window(&conn, "c", "ann", 1_000_000, &latest, Some("claude-opus-5")).unwrap();
        let at = point_for(&conn, "c", "ann", &latest).unwrap();
        assert_eq!(at, Some((500_000, "claude-opus-5".into())));

        moved(&conn, "ann", &latest, "claude-opus-6").unwrap();
        let at = point_for(&conn, "c", "ann", &latest).unwrap();
        assert_eq!(at, None, "no point from the model it ran before");
        window(&conn, "c", "ann", 200_000, &latest, Some("claude-opus-6")).unwrap();
        let at = point_for(&conn, "c", "ann", &latest).unwrap();
        assert_eq!(at, Some((160_000, "claude-opus-6".into())));
        moved(&conn, "ann", &latest, "claude-opus-6").unwrap();
        let at = point_for(&conn, "c", "ann", &latest).unwrap();
        assert_eq!(
            at.map(|(at, _)| at),
            Some(160_000),
            "the same model keeps it"
        );
    }

    #[test]
    fn the_save_notes_line_comes_once_at_75_percent_and_again_after_a_compaction() {
        let (_dir, conn) = db();
        window(&conn, "c", "ann", 1_000_000, OPUS, RAN).unwrap();
        filled(&conn, "c", "ann", 370_000).unwrap();
        assert!(
            !tell_soon(&conn, "c", "ann", OPUS).unwrap(),
            "under 75% of 500K"
        );
        filled(&conn, "c", "ann", 380_000).unwrap();
        assert!(tell_soon(&conn, "c", "ann", OPUS).unwrap());
        assert!(!tell_soon(&conn, "c", "ann", OPUS).unwrap(), "told once");
        untold(&conn, "c", "ann").unwrap();
        assert!(
            tell_soon(&conn, "c", "ann", OPUS).unwrap(),
            "told again after a compaction"
        );
        set_auto_compact(&conn, None).unwrap();
        untold(&conn, "c", "ann").unwrap();
        assert!(
            !tell_soon(&conn, "c", "ann", OPUS).unwrap(),
            "no point of its own to tell of"
        );
    }
}
