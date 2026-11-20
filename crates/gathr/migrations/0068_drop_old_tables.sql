-- The old team machinery's tables and columns, which no code reads any more: the rebuild of
-- 2026-11-20 deleted it. Children before parents; their indexes go with them.
DROP TABLE turn_answers;
DROP TABLE task_sessions;
DROP TABLE review_notes;
DROP TABLE task_after;
DROP TABLE waits;
DROP TABLE plan_parts;
DROP TABLE tasks;
DROP TABLE plan_versions;
DROP TABLE plans;
DROP TABLE held_turns;
DROP TABLE landings;
DROP TABLE changes;
DROP TABLE quiet;
DROP TABLE pr_cleanup;
DROP TABLE check_steps;
DROP TABLE check_runs;
DROP TABLE check_caches;
DROP TABLE project_checks;
DROP TABLE project_check_allowed;

-- `turns.ask_id` points at `asks`, and a column in a foreign key can't be dropped in place, so
-- `turns` is made again without the old columns, SQLite's way (foreign keys are off while
-- migrating). Each turn keeps its rowid, which orders turns of the same time. `background` points
-- at `turns` by name and finds the new one.
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
    answers TEXT,
    held TEXT CHECK (held IN ('pause', 'usage', 'update', 'claude', 'codex')),
    opened_by TEXT,
    opened_version INTEGER
) STRICT;
INSERT INTO turns_next (rowid, id, chat_id, trace_id, status, created_ts, ended_ts, why, agent_id,
                        position, answers, held, opened_by, opened_version)
    SELECT rowid, id, chat_id, trace_id, status, created_ts, ended_ts, why, agent_id,
           position, answers, held, opened_by, opened_version FROM turns;
DROP TABLE turns;
ALTER TABLE turns_next RENAME TO turns;
CREATE INDEX turns_by_chat ON turns (chat_id, created_ts);
CREATE INDEX turns_live ON turns (chat_id, status) WHERE status IN ('running', 'waiting_sign_in');
CREATE UNIQUE INDEX turns_position ON turns (chat_id, position);
CREATE INDEX turns_waiting ON turns (status) WHERE status IN ('queued', 'waiting_sign_in');

DROP TABLE asks;

-- The rest in place. `messages` is never made again: its rowids are the search index's.
ALTER TABLE messages DROP COLUMN about;
ALTER TABLE messages DROP COLUMN reply_task;
ALTER TABLE chats DROP COLUMN room;
ALTER TABLE agent_context DROP COLUMN told;
ALTER TABLE projects DROP COLUMN ci;
ALTER TABLE projects DROP COLUMN merge_method;
ALTER TABLE projects DROP COLUMN ci_source;
ALTER TABLE projects DROP COLUMN ci_set_ts;
ALTER TABLE projects DROP COLUMN merge_source;
ALTER TABLE projects DROP COLUMN merge_set_ts;
