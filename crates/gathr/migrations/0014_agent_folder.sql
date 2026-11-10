-- The agent folder (0034): each project's and agent's folder name in it, chosen the first time one is
-- needed and kept, so a rename never moves work. NULL until then.
ALTER TABLE projects ADD COLUMN folder_name TEXT;
ALTER TABLE agents ADD COLUMN folder_name TEXT;

-- Every folder Gathr makes for agents to work in, recorded before it's made, so Gathr removes it later
-- and never anything it didn't make. Not tied to the chat's row: a deleted chat's folder that was in
-- use stays here until a later sweep removes it.
CREATE TABLE agent_folders (
    path TEXT PRIMARY KEY,
    -- A git worktree, a plain folder a chat with no project runs in, or a task's temporary files.
    kind TEXT NOT NULL CHECK (kind IN ('worktree', 'folder', 'scratch')),
    chat_id TEXT NOT NULL,
    agent_id TEXT,
    -- A scratch folder's task: it goes once the task is done or cancelled. NULL for a chat's own.
    task INTEGER,
    -- The git folder a worktree is of, to unregister it there.
    project TEXT,
    made_ts INTEGER NOT NULL,
    -- When its chat was archived or deleted: it goes then.
    due_ts INTEGER,
    -- The trace of its removal: each skip and the removal itself share one reference.
    trace TEXT,
    -- Why its removal was last skipped (`in_use`, `changes`, `left`): each reason is recorded once.
    skipped TEXT,
    removed_ts INTEGER
) STRICT;

CREATE INDEX agent_folders_by_chat ON agent_folders (chat_id, agent_id);
CREATE INDEX agent_folders_open ON agent_folders (removed_ts) WHERE removed_ts IS NULL;
