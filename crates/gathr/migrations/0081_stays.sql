-- The quiet lines of the assistant's chat: an agent joined it or left it, and when. Kept only for that chat.
CREATE TABLE stays (
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    change TEXT NOT NULL CHECK (change IN ('joined', 'left')),
    ts INTEGER NOT NULL
) STRICT;
CREATE INDEX stays_by_chat ON stays (chat_id, ts);
