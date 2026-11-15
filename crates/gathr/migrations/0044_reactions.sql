-- An agent's thumbs-up on another agent's message: agreeing with no message and no turn.
-- One per agent and message: a second changes nothing. No foreign key to agents: an agent that left
-- the chat can be removed (agents.rs `remove`), and its thumbs-ups stay as the record.
CREATE TABLE reactions (
    message_id TEXT NOT NULL REFERENCES messages (id),
    agent_id TEXT NOT NULL,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    created_ts INTEGER NOT NULL,
    PRIMARY KEY (message_id, agent_id)
) STRICT;
CREATE INDEX reactions_by_chat ON reactions (chat_id);
