-- The person's tasks (F9, docs/decisions/0010-coordination-as-state.md): only a chat's Supervisor gives
-- them, and a member's need reaches the Supervisor as a `person_request` task first. A NULL owner is
-- the person, as a NULL giver already is. SQLite changes a CHECK or NOT NULL only by rebuilding the
-- table (db.rs runs migrations with foreign keys off, and checks them before each commits).

CREATE TABLE tasks_new (
    number INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    giver TEXT,
    owner TEXT,
    given_in TEXT,
    parent INTEGER REFERENCES tasks (number),
    kind TEXT NOT NULL CHECK (kind IN ('work', 'review', 'person', 'person_request')),
    what TEXT NOT NULL,
    why TEXT,
    state TEXT NOT NULL CHECK (state IN
        ('waiting', 'ready', 'working', 'held', 'done', 'blocked', 'failed', 'stopped', 'cancelled')),
    reason TEXT,
    finish TEXT CHECK (finish IN ('done', 'left', 'blocked')),
    finish_note TEXT,
    blocked_on INTEGER REFERENCES tasks (number),
    pr INTEGER,
    pr_closed INTEGER NOT NULL DEFAULT 0,
    reviewed TEXT,
    outdated INTEGER NOT NULL DEFAULT 0,
    heard INTEGER NOT NULL DEFAULT 0,
    idle_lefts INTEGER NOT NULL DEFAULT 0,
    created_ts INTEGER NOT NULL,
    updated_ts INTEGER NOT NULL,
    -- How the person answers: done, yesno, choice (one), choices (any number) or text.
    form TEXT CHECK (form IN ('done', 'yesno', 'choice', 'choices', 'text')),
    -- JSON: [{"label", "detail"}], the giver's pick first.
    choices TEXT,
    -- JSON: [{"name", "size", "path"}], copies made when it was given, under <data>/task-files.
    files TEXT,
    -- The member's `person_request` a person task was filed for: its answer goes back there.
    request INTEGER REFERENCES tasks (number),
    -- JSON: the person's answer, as sent.
    answer TEXT,
    CHECK ((owner IS NULL) = (kind = 'person'))
) STRICT;

INSERT INTO tasks_new (number, chat_id, giver, owner, given_in, parent, kind, what, why, state, reason,
                       finish, finish_note, blocked_on, pr, pr_closed, reviewed, outdated, heard,
                       idle_lefts, created_ts, updated_ts)
SELECT number, chat_id, giver, owner, given_in, parent, kind, what, why, state, reason,
       finish, finish_note, blocked_on, pr, pr_closed, reviewed, outdated, heard,
       idle_lefts, created_ts, updated_ts
FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX tasks_by_chat ON tasks (chat_id, number);
CREATE INDEX tasks_open ON tasks (state) WHERE state NOT IN ('done', 'cancelled');
CREATE INDEX tasks_person_open ON tasks (created_ts) WHERE kind = 'person' AND state = 'ready';
