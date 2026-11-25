-- A background command's kind as the engine names it (`local_bash`, `local_agent`, ...), and once the engine says it
-- ended: how (`done`, `failed`, `stopped`) and the file its output went to. The row stays until the engine no longer
-- lists it.
ALTER TABLE background ADD COLUMN kind TEXT;
ALTER TABLE background ADD COLUMN status TEXT CHECK (status IN ('done', 'failed', 'stopped'));
ALTER TABLE background ADD COLUMN output TEXT;

-- Background commands that ended, for the chat's Background tab: the newest 100 of each chat are kept, and the person's
-- Clear removes them. A chat's delete removes its rows.
CREATE TABLE background_ended (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL REFERENCES turns (id),
    description TEXT NOT NULL,
    kind TEXT,
    started_ts INTEGER NOT NULL,
    ended_ts INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('done', 'failed', 'stopped')),
    output TEXT
) STRICT;
CREATE INDEX background_ended_turn ON background_ended (turn_id);
