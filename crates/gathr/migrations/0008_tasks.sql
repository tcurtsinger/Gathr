-- Tasks and waits (F5, docs/decisions/0010-coordination-as-state.md).

-- Work one agent owes in a chat. The number is one sequence across chats. `given_in` is the turn
-- that gave it, so its owner's work sits under the message that asked for it (the inline thread).
CREATE TABLE tasks (
    number INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    giver TEXT,
    owner TEXT NOT NULL,
    given_in TEXT,
    parent INTEGER REFERENCES tasks (number),
    kind TEXT NOT NULL CHECK (kind IN ('work', 'review')),
    what TEXT NOT NULL,
    why TEXT,
    state TEXT NOT NULL CHECK (state IN
        ('waiting', 'ready', 'working', 'held', 'done', 'blocked', 'failed', 'stopped', 'cancelled')),
    -- Held, blocked, failed or cancelled: on what, and why. Done: the result.
    reason TEXT,
    -- What its owner said as its turn ended, applied when the turn has ended: done, left or blocked.
    finish TEXT CHECK (finish IN ('done', 'left', 'blocked')),
    finish_note TEXT,
    -- Blocked until this task is done (G52), not by a new one.
    blocked_on INTEGER REFERENCES tasks (number),
    -- Its pull request: merged settles it, closed cancels it (G67, G44).
    pr INTEGER,
    -- Its PR merged or closed: looked at no more (a done review's PR is followed for G73).
    pr_closed INTEGER NOT NULL DEFAULT 0,
    -- A review's commit (G73), and whether its PR has moved past it.
    reviewed TEXT,
    outdated INTEGER NOT NULL DEFAULT 0,
    -- Its giver has heard back on it: once the turn that gave it has ended and every task given
    -- in that turn is settled.
    heard INTEGER NOT NULL DEFAULT 0,
    -- `left` by turns that changed nothing in a row (0010: twice is blocked).
    idle_lefts INTEGER NOT NULL DEFAULT 0,
    created_ts INTEGER NOT NULL,
    updated_ts INTEGER NOT NULL
) STRICT;

CREATE INDEX tasks_by_chat ON tasks (chat_id, number);
CREATE INDEX tasks_open ON tasks (state) WHERE state NOT IN ('done', 'cancelled');

CREATE TABLE task_after (
    task INTEGER NOT NULL REFERENCES tasks (number),
    after_task INTEGER NOT NULL REFERENCES tasks (number),
    PRIMARY KEY (task, after_task)
) STRICT;

-- The task a turn works on, if any.
ALTER TABLE turns ADD COLUMN task INTEGER;
CREATE INDEX turns_by_task ON turns (task) WHERE task IS NOT NULL;

-- An agent waiting instead of looping. `part_of` joins the parts of an `all` wait to it. `target` is the
-- agent waited on, a PR's number, a commit, or a check's command.
CREATE TABLE waits (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    task INTEGER REFERENCES tasks (number),
    left_in TEXT,
    part_of TEXT REFERENCES waits (id),
    kind TEXT NOT NULL CHECK (kind IN ('agent', 'ci', 'pr', 'check', 'all')),
    target TEXT,
    repo TEXT,
    cwd TEXT,
    then_do TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'met', 'fired', 'picked', 'expired', 'cancelled')),
    -- How it was met or ended, in a line for the agent.
    line TEXT,
    -- A CI look's head commit and since when (the no-checks grace starts again on a push).
    head TEXT,
    head_ts INTEGER,
    failures INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    looked_ts INTEGER,
    until_ts INTEGER NOT NULL,
    created_ts INTEGER NOT NULL,
    ended_ts INTEGER
) STRICT;

CREATE INDEX waits_open ON waits (state) WHERE state IN ('open', 'met');
CREATE INDEX waits_by_chat ON waits (chat_id, created_ts);

-- What Gathr told an agent (a task, a wake-up, a hear-back) is a message of its own role, never the
-- person's. SQLite changes a CHECK only by rebuilding the table.
CREATE TABLE messages_new (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    turn_id TEXT,
    ts INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'gathr')),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'tool')),
    body TEXT NOT NULL
) STRICT;
INSERT INTO messages_new SELECT id, chat_id, turn_id, ts, role, kind, body FROM messages;
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;
CREATE INDEX messages_by_chat ON messages (chat_id, ts);
