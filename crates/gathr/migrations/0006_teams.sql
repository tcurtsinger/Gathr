-- Chats for a team (F4, 0013, 0028): a chat's project folder, set when it's made; its agents and
-- their roles; and each turn's place in the chat, taken when it starts, so replies from several
-- agents keep the order their turns started in.
ALTER TABLE chats ADD COLUMN project TEXT;

CREATE TABLE chat_members (
    chat_id TEXT NOT NULL REFERENCES chats (id),
    agent_id TEXT NOT NULL REFERENCES agents (id),
    role TEXT NOT NULL CHECK (role IN ('supervisor', 'reviewer', 'member')),
    added_ts INTEGER NOT NULL,
    PRIMARY KEY (chat_id, agent_id)
) STRICT;

ALTER TABLE turns ADD COLUMN position INTEGER;

-- Turns that have started (or ended) until now take their places in the order they were made.
UPDATE turns SET position = (
    SELECT count(*) FROM turns e
    WHERE e.chat_id = turns.chat_id AND e.status NOT IN ('queued', 'waiting_sign_in')
      AND (e.created_ts < turns.created_ts OR (e.created_ts = turns.created_ts AND e.id <= turns.id)))
WHERE status NOT IN ('queued', 'waiting_sign_in');

CREATE UNIQUE INDEX turns_position ON turns (chat_id, position);
