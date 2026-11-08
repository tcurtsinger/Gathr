-- Agents (0026): each runs on one account, with its model and effort. NULL model or effort means
-- the engine's own default, so nothing needs editing when the engine changes it.
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- The name in Unicode lower case, from Rust: SQLite's NOCASE folds only ASCII.
    name_key TEXT NOT NULL,
    -- An account isn't removed while an agent uses it.
    account_id TEXT NOT NULL REFERENCES accounts (id),
    model TEXT,
    effort TEXT,
    created_ts INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX agents_by_name ON agents (name_key);
CREATE INDEX agents_by_account ON agents (account_id);

-- The agent a turn runs as; NULL for a turn sent to no agent, which runs on the oldest account.
-- No foreign key: a deleted agent's turns keep its id for the record.
ALTER TABLE turns ADD COLUMN agent_id TEXT;

-- The account the chat's engine session lives in: a session can only resume in its own login
-- folder, so a turn on another account starts a new one.
ALTER TABLE chats ADD COLUMN engine_account TEXT;

-- Until now every chat ran on the oldest account, and a new oldest one cleared every session, so
-- a session left from before lives in the oldest account's folder.
UPDATE chats SET engine_account = (SELECT id FROM accounts ORDER BY created_ts, id LIMIT 1)
WHERE engine_session IS NOT NULL;
