-- Pause and stop, usage holds and background commands (F10, docs/decisions/0010-coordination-as-state.md).

-- A pause of the whole app (target ''), one chat or one agent. Running turns finish; nothing new
-- starts on its own until it's resumed.
CREATE TABLE pauses (
    scope TEXT NOT NULL CHECK (scope IN ('app', 'chat', 'agent')),
    target TEXT NOT NULL,
    since_ts INTEGER NOT NULL,
    -- When the service said nothing in it runs any more; NULL while something does.
    quiet_ts INTEGER,
    -- The person's Stop now: what was running was ended, and carry-ons wait for Restart.
    stopped INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, target)
) STRICT;

-- An account out of usage until a time its engine gave: its turns wait until 30 s after.
CREATE TABLE usage_holds (
    account_id TEXT PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
    until_ts INTEGER NOT NULL,
    since_ts INTEGER NOT NULL,
    -- The engine's name for the limit: five_hour, seven_day, …
    kind TEXT
) STRICT;

-- Commands an engine runs in the background, by the engine's own id, under the turn that started
-- them. Emptied at each start: they end with their engine.
CREATE TABLE background (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL REFERENCES turns (id),
    description TEXT NOT NULL,
    started_ts INTEGER NOT NULL
) STRICT;

-- What holds a queued turn back: 'pause' or 'usage'; NULL when only its place in the queue does.
ALTER TABLE turns ADD COLUMN held TEXT CHECK (held IN ('pause', 'usage'));
