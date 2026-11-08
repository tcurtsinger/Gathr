-- One engine per agent (F2, 0010 Sessions): each agent keeps its own session in each chat. The
-- agent key is the agent's id, or '' for turns sent to no agent. A session resumes only in the
-- login folder it was made in, so its account stays with it.
CREATE TABLE engine_sessions (
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_key TEXT NOT NULL,
    account_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    PRIMARY KEY (chat_id, agent_key)
) STRICT;

-- Until now a chat had one session, whichever agent ran: it's the session of its last turn's agent.
INSERT INTO engine_sessions (chat_id, agent_key, account_id, session_id)
SELECT c.id,
       COALESCE((SELECT t.agent_id FROM turns t WHERE t.chat_id = c.id
                 ORDER BY t.created_ts DESC, t.id DESC LIMIT 1), ''),
       c.engine_account, c.engine_session
FROM chats c WHERE c.engine_session IS NOT NULL AND c.engine_account IS NOT NULL;

ALTER TABLE chats DROP COLUMN engine_session;
ALTER TABLE chats DROP COLUMN engine_account;
