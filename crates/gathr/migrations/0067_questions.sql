-- A question an agent of a room chat asks the person with `ask_person`: open until they answer it with one of
-- its choices, their own words or both. Older chats keep theirs as tasks. `choices` is a JSON array of
-- `{label, detail}`, null when it offers none. No foreign key to agents, as with reactions.
CREATE TABLE questions (
    id INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent TEXT NOT NULL,
    question TEXT NOT NULL,
    choices TEXT,
    pick TEXT,
    words TEXT,
    asked_ts INTEGER NOT NULL,
    answered_ts INTEGER
) STRICT;
CREATE INDEX questions_by_chat ON questions (chat_id);
