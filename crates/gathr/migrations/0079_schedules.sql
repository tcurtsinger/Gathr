-- Scheduled work: words an agent's `schedule` call keeps, which come into its chat as a message to that agent at
-- their time on this PC's clock. A repeating one has `minute` (of the day) and `days` (the weekdays it runs, as
-- SQLite's %w numbers them, '12345' for weekdays); a once has `once_ts`. `next_ts` is when it is next due, none while
-- it is disabled.
CREATE TABLE schedules (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    words TEXT NOT NULL,
    minute INTEGER,
    days TEXT,
    once_ts INTEGER,
    next_ts INTEGER,
    created_ts INTEGER NOT NULL
) STRICT;
CREATE INDEX schedules_due ON schedules (next_ts) WHERE next_ts IS NOT NULL;

-- Each time one came due: `kind` 'on_time', 'late', 'now' (Run now) or 'skipped'; the turn and the message it
-- queued (none when skipped). A run outlives its schedule: the chat's message keeps saying it was scheduled.
CREATE TABLE schedule_runs (
    id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    kind TEXT NOT NULL CHECK (kind IN ('on_time', 'late', 'now', 'skipped')),
    due_ts INTEGER NOT NULL,
    ran_ts INTEGER NOT NULL,
    turn_id TEXT,
    message_id TEXT
) STRICT;
CREATE INDEX schedule_runs_by_schedule ON schedule_runs (schedule_id, ran_ts);
CREATE INDEX schedule_runs_by_message ON schedule_runs (message_id) WHERE message_id IS NOT NULL;
