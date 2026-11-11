-- A wait for a PR to be up to date with its base (0036 C12, owner's task 1450): `up_to_date`, a PR
-- number as its target. SQLite can't change a CHECK, so the table is made again with every row.
CREATE TABLE waits_next (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    task INTEGER REFERENCES tasks (number),
    left_in TEXT,
    part_of TEXT REFERENCES waits (id),
    kind TEXT NOT NULL CHECK (kind IN ('agent', 'ci', 'pr', 'up_to_date', 'check', 'all')),
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

INSERT INTO waits_next (id, chat_id, agent_id, task, left_in, part_of, kind, target, repo, cwd, then_do,
                        state, line, head, head_ts, failures, last_error, looked_ts, until_ts, created_ts,
                        ended_ts)
SELECT id, chat_id, agent_id, task, left_in, part_of, kind, target, repo, cwd, then_do, state, line, head,
       head_ts, failures, last_error, looked_ts, until_ts, created_ts, ended_ts
FROM waits;

DROP TABLE waits;
ALTER TABLE waits_next RENAME TO waits;

CREATE INDEX waits_open ON waits (state) WHERE state IN ('open', 'met');
CREATE INDEX waits_by_chat ON waits (chat_id, created_ts);
