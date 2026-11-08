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

-- Until now a chat had one session, whichever agent ran: it's the session of the agent whose engine
-- ran last. Only a turn whose engine ran has its frames; a newer turn that failed before its engine
-- started, or never started, didn't touch the session. A chat with no such turn keeps none.
INSERT INTO engine_sessions (chat_id, agent_key, account_id, session_id)
SELECT c.id, COALESCE(t.agent_id, ''), c.engine_account, c.engine_session
FROM chats c
JOIN turns t ON t.id = (
    SELECT n.id FROM turns n
    WHERE n.chat_id = c.id
      AND EXISTS (SELECT 1 FROM engine_raw r WHERE r.turn_id = n.id AND r.direction = 'out')
    ORDER BY n.created_ts DESC, n.id DESC LIMIT 1)
WHERE c.engine_session IS NOT NULL AND c.engine_account IS NOT NULL;

ALTER TABLE chats DROP COLUMN engine_session;
ALTER TABLE chats DROP COLUMN engine_account;
