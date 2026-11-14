-- Each task's engine session for the agent that runs it (0055). A session resumes only in the
-- login folder it was made in, so its account stays with it.
CREATE TABLE task_sessions (
    task INTEGER NOT NULL REFERENCES tasks (number),
    agent_id TEXT NOT NULL,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    account_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    -- The context it last filled and whether it was told it is near compaction, so both follow it back.
    used INTEGER,
    told INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (task, agent_id)
) STRICT;

-- A task already under way ran in its owner's one session: it stays there. A lead's task gets a
-- row too; the lead never reads it.
INSERT INTO task_sessions (task, agent_id, chat_id, account_id, session_id)
SELECT t.number, t.owner, t.chat_id, s.account_id, s.session_id
FROM tasks t JOIN engine_sessions s ON s.chat_id = t.chat_id AND s.agent_key = t.owner
WHERE t.state NOT IN ('done', 'cancelled')
  AND EXISTS (SELECT 1 FROM turns u WHERE u.task = t.number
              AND u.status NOT IN ('queued', 'waiting_sign_in'));
