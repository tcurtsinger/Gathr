-- Stage 3, part H (0062): the lead's plans. A plan belongs to an ask; each version is one of the
-- lead's messages in the chat, with its parts; Approve makes a task of each part of one version.
-- A plan's state is read from these rows and its check task, never stored apart.
CREATE TABLE plans (
    id INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    ask_id TEXT NOT NULL REFERENCES asks (id),
    -- The Supervisor that wrote it: it gives Approve's tasks, and a send-back goes to it.
    lead TEXT NOT NULL,
    created_ts INTEGER NOT NULL
) STRICT;
CREATE INDEX plans_by_ask ON plans (ask_id);
CREATE INDEX plans_by_chat ON plans (chat_id);
CREATE TABLE plan_versions (
    id INTEGER PRIMARY KEY,
    plan_id INTEGER NOT NULL REFERENCES plans (id),
    n INTEGER NOT NULL,
    -- The lead's message that is this version, and the turn that wrote it, where Approve gives its
    -- tasks. No foreign keys: deleting a chat removes its messages and turns before its plans.
    message_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    title TEXT NOT NULL,
    text TEXT NOT NULL,
    -- The lead's answer to each finding on the version before: JSON [{"note", "changed", "why"}].
    answers TEXT,
    created_ts INTEGER NOT NULL,
    -- Gathr's line saying how its check ended, and when the version reached the person: never
    -- while its check runs, nor when the check left findings.
    check_line TEXT,
    sent_ts INTEGER,
    -- The person's word on it, the words of a send-back, and Gathr's line or turn saying it.
    decided TEXT CHECK (decided IN ('approved', 'sent_back', 'dropped')),
    decided_ts INTEGER,
    words TEXT,
    decided_line TEXT,
    UNIQUE (plan_id, n)
) STRICT;
CREATE TABLE plan_parts (
    version_id INTEGER NOT NULL REFERENCES plan_versions (id),
    -- Its number in this version: its place, and what `after_parts` names.
    n INTEGER NOT NULL,
    -- The agent it is for. No foreign key: an agent can be removed, and the record stays.
    owner TEXT NOT NULL,
    what TEXT NOT NULL,
    why TEXT NOT NULL,
    done TEXT NOT NULL,
    proof TEXT NOT NULL,
    place TEXT NOT NULL,
    size TEXT NOT NULL,
    size_lines INTEGER NOT NULL,
    -- JSON arrays: the earlier parts of this version, and the tasks of this chat, it comes after.
    after_parts TEXT NOT NULL DEFAULT '[]',
    after_tasks TEXT NOT NULL DEFAULT '[]',
    -- The task Approve made of it.
    task INTEGER REFERENCES tasks (number),
    PRIMARY KEY (version_id, n)
) STRICT;
CREATE INDEX plan_parts_by_task ON plan_parts (task) WHERE task IS NOT NULL;
-- A plan check: the version it checks.
ALTER TABLE tasks ADD COLUMN checks INTEGER REFERENCES plan_versions (id);
CREATE INDEX tasks_checking ON tasks (checks) WHERE checks IS NOT NULL;
