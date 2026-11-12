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

/// The agent's model holds this much, seen on `of` (`model_key`): its context as now shown.
pub fn window(
    conn: &Connection,
    chat: &str,
    agent: &str,
    window: i64,
    seen_on: &str,
) -> Result<Value, Error> {
    conn.execute(
        "INSERT INTO agent_context (chat_id, agent_id, window, window_of, updated_ts) VALUES (?1, ?2, ?3, ?5, ?4)
         ON CONFLICT (chat_id, agent_id) DO UPDATE SET window = ?3, window_of = ?5, updated_ts = ?4",
        params![chat, agent, window, now_us(), seen_on],
    )?;
    Ok(of(conn, chat, agent)?.unwrap_or(Value::Null))
}

/// `{used, window, compact_at}`, each null until known; none before the agent's first call.
pub fn of(conn: &Connection, chat: &str, agent: &str) -> Result<Option<Value>, Error> {
    let found: Option<(Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT used, window FROM agent_context WHERE chat_id = ?1 AND agent_id = ?2",
            params![chat, agent],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    // Unknown until the model's window is: as the engine is started (`point_for`).
    let window = found.and_then(|(_, window)| window);
    let at = match window {
        Some(window) => point(auto_compact(conn)?, Some(window)),
        None => None,
    };
    Ok(found.map(|(used, window)| json!({ "used": used, "window": window, "compact_at": at })))
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

/// Which engine and model a window was seen on: the agent's model as set (`None` the engine's own),
/// so a changed model or account never takes another model's window.
pub fn model_key(engine: &str, model: Option<&str>) -> String {
    format!("{engine}:{}", model.unwrap_or_default())
}

/// The point for the agent's engine about to start in `chat` on `of` (`model_key`): with the window
/// seen on that model, in this chat or else its latest anywhere. None while that window isn't known,
/// so the engine compacts at its own point rather than one past its window (#168's review).
pub fn point_for(
    conn: &Connection,
    chat: &str,
    agent: &str,
    of: &str,
) -> Result<Option<i64>, Error> {
    let window: Option<i64> = conn
        .query_row(
            "SELECT window FROM agent_context WHERE agent_id = ?2 AND window IS NOT NULL AND window_of = ?3
             ORDER BY chat_id = ?1 DESC, updated_ts DESC LIMIT 1",
            params![chat, agent, of],
            |r| r.get(0),
        )
        .optional()?;
    let Some(window) = window else {
        return Ok(None);
    };
    Ok(point(auto_compact(conn)?, Some(window)))
}

/// The line a turn is given once its agent's conversation nears where it compacts (K3).
pub const SOON: &str = "Your conversation is close to its compaction point: when it compacts, earlier detail goes. \
    Write down what you still need (your notes, the task, the plan) in your own folder now.";

/// Whether the agent's next turn in `chat` gets `SOON`: its last call filled 75% of where it compacts,
/// and it hasn't been told since its last compaction. Marked told when it does.
pub fn tell_soon(conn: &Connection, chat: &str, agent: &str, of: &str) -> Result<bool, Error> {
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
    let Some(at) = point_for(conn, chat, agent, of)? else {
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
            "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 0), ('d', 'Other', 0);",
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

    /// What members show is what an engine starts with: no point until the window is known.
    #[test]
    fn the_point_shown_is_unknown_until_the_window_is() {
        let (_dir, conn) = db();
        let shown = filled(&conn, "c", "ann", 1_000).unwrap();
        assert!(shown["compact_at"].is_null(), "{shown}");
        let shown = window(&conn, "c", "ann", 200_000, "claude:").unwrap();
        assert_eq!(shown["compact_at"], 160_000, "{shown}");
    }

    /// The window an engine starts with: its chat's, else the agent's latest anywhere, on its model.
    #[test]
    fn a_new_chat_takes_the_agents_last_window_on_the_same_model() {
        let (_dir, conn) = db();
        assert_eq!(
            point_for(&conn, "c", "ann", "claude:").unwrap(),
            None,
            "none seen yet"
        );
        window(&conn, "d", "ann", 200_000, "claude:").unwrap();
        assert_eq!(
            point_for(&conn, "c", "ann", "claude:").unwrap(),
            Some(160_000)
        );
        let other = model_key("claude", Some("opus"));
        assert_eq!(
            point_for(&conn, "c", "ann", &other).unwrap(),
            None,
            "another model"
        );
        window(&conn, "c", "ann", 1_000_000, "claude:").unwrap();
        assert_eq!(
            point_for(&conn, "c", "ann", "claude:").unwrap(),
            Some(500_000)
        );
    }

    #[test]
    fn the_save_notes_line_comes_once_at_75_percent_and_again_after_a_compaction() {
        let (_dir, conn) = db();
        window(&conn, "c", "ann", 1_000_000, "claude:").unwrap();
        filled(&conn, "c", "ann", 370_000).unwrap();
        assert!(
            !tell_soon(&conn, "c", "ann", "claude:").unwrap(),
            "under 75% of 500K"
        );
        filled(&conn, "c", "ann", 380_000).unwrap();
        assert!(tell_soon(&conn, "c", "ann", "claude:").unwrap());
        assert!(
            !tell_soon(&conn, "c", "ann", "claude:").unwrap(),
            "told once"
        );
        untold(&conn, "c", "ann").unwrap();
        assert!(
            tell_soon(&conn, "c", "ann", "claude:").unwrap(),
            "told again after a compaction"
        );
        set_auto_compact(&conn, None).unwrap();
        untold(&conn, "c", "ann").unwrap();
        assert!(
            !tell_soon(&conn, "c", "ann", "claude:").unwrap(),
            "no point of its own to tell of"
        );
    }
}
