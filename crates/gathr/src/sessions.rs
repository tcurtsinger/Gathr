//! The agent's conversation in a chat (`engine_sessions`): every turn runs in the agent's latest.

use rusqlite::{Connection, params};

use crate::Error;

/// The compacted conversation keeps the marker until its next opening.
pub fn summed(
    conn: &Connection,
    chat: &str,
    agent: &str,
    account: &str,
    session: &str,
    summed: bool,
) -> Result<(), Error> {
    conn.execute(
        "UPDATE engine_sessions SET summed = ?5
         WHERE chat_id = ?1 AND agent_key = ?2 AND account_id = ?3 AND session_id = ?4",
        params![chat, agent, account, session, summed],
    )?;
    Ok(())
}

/// Whether this conversation still needs its post-compaction opening.
pub fn is_summed(
    conn: &Connection,
    chat: &str,
    agent: &str,
    account: &str,
    session: &str,
) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM engine_sessions WHERE chat_id = ?1 AND agent_key = ?2
             AND account_id = ?3 AND session_id = ?4 AND summed = 1)",
        params![chat, agent, account, session],
        |r| r.get(0),
    )?)
}

/// The engine refused to resume `session` (N8): `agent`'s row holding it goes, so the next start
/// opens new. `agent` none is a chat's turns sent to no agent.
pub fn forget(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    session: &str,
) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM engine_sessions WHERE chat_id = ?1 AND agent_key = ?2 AND session_id = ?3",
        params![chat, agent.unwrap_or_default(), session],
    )?;
    Ok(())
}
