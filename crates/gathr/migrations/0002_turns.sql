-- Chats with an engine (docs/first-slice.md): their session, their turns, and the raw frames.

-- The engine's own session id, so the next message after a restart continues it.
ALTER TABLE chats ADD COLUMN engine_session TEXT;

-- One row per message the person sends to the engine. A message sent while a turn runs joins
-- that turn (steered in), so it gets no row of its own.
CREATE TABLE turns (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    trace_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'waiting_sign_in', 'running', 'done', 'stopped', 'failed')),
    created_ts INTEGER NOT NULL,
    ended_ts INTEGER,
    why TEXT
) STRICT;

CREATE INDEX turns_by_chat ON turns (chat_id, created_ts);
CREATE INDEX turns_waiting ON turns (status) WHERE status IN ('queued', 'waiting_sign_in');

-- Every frame in and out of an engine, redacted before it's written (0011).
CREATE TABLE engine_raw (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL,
    chat_id TEXT,
    turn_id TEXT,
    pid INTEGER,
    direction TEXT NOT NULL CHECK (direction IN ('in', 'out', 'err')),
    frame_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    malformed INTEGER NOT NULL DEFAULT 0,
    body TEXT
) STRICT;

CREATE INDEX engine_raw_by_turn ON engine_raw (turn_id, id);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
