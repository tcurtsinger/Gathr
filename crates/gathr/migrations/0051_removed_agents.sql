-- Deleting an agent keeps its row, marked removed (0069): the chats it spoke in still name it, every
-- list leaves it out, and its name is free for a new agent. A removed agent has no account, so the
-- account it ran on can be removed. SQLite can't drop a column's NOT NULL, so the table is made again
-- with every row, as are the folders' records below.
CREATE TABLE agents_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- The name in Unicode lower case, from Rust: SQLite's NOCASE folds only ASCII.
    name_key TEXT NOT NULL,
    -- An account isn't removed while an agent uses it. NULL once the agent is removed.
    account_id TEXT REFERENCES accounts (id),
    model TEXT,
    effort TEXT,
    created_ts INTEGER NOT NULL,
    approvals TEXT NOT NULL DEFAULT 'auto' CHECK (approvals IN ('ask', 'edits', 'auto', 'full')),
    folder_name TEXT,
    removed_ts INTEGER,
    CHECK ((account_id IS NULL) = (removed_ts IS NOT NULL))
) STRICT;

INSERT INTO agents_next (id, name, name_key, account_id, model, effort, created_ts, approvals, folder_name)
SELECT id, name, name_key, account_id, model, effort, created_ts, approvals, folder_name
FROM agents;

DROP TABLE agents;
ALTER TABLE agents_next RENAME TO agents;

-- Unique ignoring case among the agents that aren't removed. A removed agent keeps its folder name,
-- so a new agent of the same name never works in the old one's folder.
CREATE UNIQUE INDEX agents_by_name ON agents (name_key) WHERE removed_ts IS NULL;
CREATE INDEX agents_by_account ON agents (account_id);

-- A deleted project's own folders (its agents' folders, its checks checkout and caches) belong to no
-- chat: they're recorded with none, and the sweep removes them as it removes a deleted chat's.
CREATE TABLE agent_folders_next (
    path TEXT PRIMARY KEY,
    -- A git worktree, a plain folder a chat with no project runs in, or an engine's temporary files.
    kind TEXT NOT NULL CHECK (kind IN ('worktree', 'folder', 'scratch')),
    -- NULL for a deleted project's own folders.
    chat_id TEXT,
    agent_id TEXT,
    -- The git folder a worktree is of, to unregister it there.
    project TEXT,
    made_ts INTEGER NOT NULL,
    -- When it was due to go: its chat or project deleted, its chat archived, or, for temporary files,
    -- its engine closed.
    due_ts INTEGER,
    -- The trace of its removal: each skip and the removal itself share one reference.
    trace TEXT,
    -- Why its removal was last skipped (`in_use`, `changes`, `left`): each reason is recorded once.
    skipped TEXT,
    removed_ts INTEGER
) STRICT;

INSERT INTO agent_folders_next (path, kind, chat_id, agent_id, project, made_ts, due_ts, trace, skipped,
                                removed_ts)
SELECT path, kind, chat_id, agent_id, project, made_ts, due_ts, trace, skipped, removed_ts
FROM agent_folders;

DROP TABLE agent_folders;
ALTER TABLE agent_folders_next RENAME TO agent_folders;

CREATE INDEX agent_folders_by_chat ON agent_folders (chat_id, agent_id);
CREATE INDEX agent_folders_open ON agent_folders (removed_ts) WHERE removed_ts IS NULL;
