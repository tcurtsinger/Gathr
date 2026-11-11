-- Agents message each other (plans/agent-messages.md): a message an agent sent to one member is
-- saved as the sender's, on its turn, with whom it's for.
ALTER TABLE messages ADD COLUMN to_agent TEXT;
-- What Gathr told the member, in its own turn, points at the message it carries, so the chat shows
-- the words once.
ALTER TABLE messages ADD COLUMN copy_of TEXT;

-- Each message between agents is a hand-off (0010's counter). Its depth is the sender's chain
-- plus one; a chain starts over after the person's message in the chat or a task there settling.
CREATE TABLE handoffs (
    id INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL,
    depth INTEGER NOT NULL,
    created_ts INTEGER NOT NULL
) STRICT;

CREATE INDEX handoffs_to ON handoffs (chat_id, to_agent, created_ts);
