-- A first engine download holds turns instead of borrowing a program from PATH (0025).
-- Migrations run with foreign keys off and validate every reference before committing.
CREATE TABLE turns_next (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    trace_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'waiting_sign_in', 'running', 'done', 'stopped', 'failed')),
    created_ts INTEGER NOT NULL,
    ended_ts INTEGER,
    why TEXT,
    agent_id TEXT,
    position INTEGER,
    task INTEGER,
    answers TEXT,
    held TEXT CHECK (held IN ('pause', 'usage', 'update', 'claude', 'codex')),
    stop_why TEXT
) STRICT;
INSERT INTO turns_next SELECT * FROM turns;
DROP TABLE turns;
ALTER TABLE turns_next RENAME TO turns;
CREATE INDEX turns_by_chat ON turns (chat_id, created_ts);
CREATE INDEX turns_waiting ON turns (status) WHERE status IN ('queued', 'waiting_sign_in');
CREATE UNIQUE INDEX turns_position ON turns (chat_id, position);
CREATE INDEX turns_by_task ON turns (task) WHERE task IS NOT NULL;
