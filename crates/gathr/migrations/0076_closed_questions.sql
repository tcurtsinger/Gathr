-- Archiving a chat closes its open questions unanswered (the owner, 2026-11-21): they no longer need the person,
-- and Restore leaves them closed. A chat archived before keeps its questions closed from when it was archived.
ALTER TABLE questions ADD COLUMN closed_ts INTEGER;
UPDATE questions SET closed_ts = (SELECT c.archived_ts FROM chats c WHERE c.id = questions.chat_id)
WHERE answered_ts IS NULL AND chat_id IN (SELECT id FROM chats WHERE archived_ts IS NOT NULL);
