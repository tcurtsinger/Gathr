-- Stage 3, part I (0063): a held turn's worktree at its start, and what its end found changed.
-- No foreign keys to turns or asks: chat deletion removes turns before this table goes.
CREATE TABLE held_turns (
    turn_id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    ask_id TEXT,
    -- The look-into task the turn was on, if one.
    task INTEGER,
    worktree TEXT NOT NULL,
    -- At its start: the head commit, and git status's entries with their content ids, as JSON
    -- [["XY path", "id" | null], ...].
    head TEXT,
    status TEXT NOT NULL,
    started_ts INTEGER NOT NULL,
    -- At its end (or at the agent's next turn, after a stop): the files found changed (JSON), or
    -- NULL for none or when the turn was no longer held; Gathr's line; when the agent was told.
    looked_ts INTEGER,
    changed TEXT,
    line TEXT,
    told_ts INTEGER
) STRICT;
CREATE INDEX held_turns_unlooked ON held_turns (chat_id, agent_id) WHERE looked_ts IS NULL;
CREATE INDEX held_turns_lines ON held_turns (chat_id) WHERE line IS NOT NULL;
