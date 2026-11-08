-- Named accounts (0002, 0026). Each keeps its login in <data>/accounts/<id>/<engine>, named by
-- its id, so a rename never moves a login.
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    engine TEXT NOT NULL CHECK (engine IN ('claude', 'codex')),
    name TEXT NOT NULL,
    -- The name in Unicode lower case, from Rust: SQLite's NOCASE folds only ASCII.
    name_key TEXT NOT NULL,
    created_ts INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX accounts_by_name ON accounts (name_key);

-- The one account from before accounts had names: its folder is accounts/main/claude already,
-- so its login carries over with no new sign-in.
INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('main', 'claude', 'Main', 'main', 0);

-- Claude Code's path is no longer a setting: GATHR_CLAUDE_PATH stands in for it in dev and tests.
DELETE FROM settings WHERE key = 'engines.claude.path';
