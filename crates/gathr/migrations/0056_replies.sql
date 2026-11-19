-- A reply the person typed in a thread (0076): what it answers. ADD COLUMN rewrites no row, so every rowid, and the
-- search index that points at them (0015), stay as they are. Null for every other message, and for one from before.
-- A later migration that rebuilds `messages` must carry both, as 0028 carried `to_agent` and `copy_of`.
-- The message it answers: a text message of the same chat; for Gathr's copy of a message, the message it carries.
ALTER TABLE messages ADD COLUMN reply_to TEXT;
-- The task it is about, when it answers a task's message: a task's message has no message of its own.
ALTER TABLE messages ADD COLUMN reply_task INTEGER;
