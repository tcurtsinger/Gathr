-- Stage 3, part G (0061): the person's asks in a team chat, and their size. An ask is named by the
-- person's message that started it. NULL size: not sized yet, and it counts as new.
CREATE TABLE asks (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    size TEXT CHECK (size IN ('repair', 'new')),
    -- Who sized it last: the lead's agent id, or 'person'.
    sized_by TEXT,
    -- The lead's turn that sized it last: its line sits after that turn.
    sized_in TEXT,
    sized_ts INTEGER,
    -- Gathr's "Fixed:" line, once said: that message's id.
    fixed TEXT
) STRICT;
CREATE INDEX asks_by_chat ON asks (chat_id, id);
-- The ask each turn and task serves. Rows from before have none.
ALTER TABLE turns ADD COLUMN ask_id TEXT REFERENCES asks (id);
ALTER TABLE tasks ADD COLUMN ask_id TEXT REFERENCES asks (id);
CREATE INDEX turns_by_ask ON turns (ask_id) WHERE ask_id IS NOT NULL;
CREATE INDEX tasks_by_ask ON tasks (ask_id) WHERE ask_id IS NOT NULL;
