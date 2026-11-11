-- How full each agent's context is in each chat (compaction plan, K1): its last model call's prompt
-- and its model's window, as the engine reports them.
CREATE TABLE agent_context (
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL,
    used INTEGER,
    window INTEGER,
    updated_ts INTEGER NOT NULL,
    PRIMARY KEY (chat_id, agent_id)
) STRICT;
