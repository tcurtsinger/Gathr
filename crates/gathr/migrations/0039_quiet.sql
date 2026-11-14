-- A quiet chat is noticed, and its dead time has a number (0052). One row for a chat: the stretch
-- it is stuck in now, the count the person is told from, and what they and the lead were last told
-- of. A stretch that ended is an event, `gathr.quiet.stretch`. Times are counted check by check, so
-- time the service was down adds nothing. A key is a line of the waiting answer without its words:
-- `[kind, task, on, agent, since_us]`.
CREATE TABLE quiet (
    chat_id    TEXT PRIMARY KEY REFERENCES chats (id),
    since_ts   INTEGER,                    -- this stretch began; NULL: none open
    stretch_us INTEGER NOT NULL DEFAULT 0, -- stuck time counted in it
    counted_ts INTEGER,                    -- counted up to; NULL: the clock is stopped
    what       TEXT,                       -- JSON list of what wasn't moving, last check
    total_us   INTEGER,                    -- stuck time in this count; NULL: nothing not moving is new to the person
    wake_turn  TEXT,                       -- the lead's wake in this count
    told_ts    INTEGER,                    -- the person was told; cleared when that stretch ends
    reset_ts   INTEGER NOT NULL DEFAULT 0, -- the person's message the count runs from
    told_what  TEXT,                       -- JSON list of keys: what wasn't moving when the person was last told
    woke_what  TEXT                        -- JSON list of keys: the lead's news at its last wake
) STRICT;

-- When the person last wrote in a chat, in one seek: asked at every check, and for every message
-- between agents (`handoffs::record`).
CREATE INDEX messages_person ON messages (chat_id, ts) WHERE role = 'user';
