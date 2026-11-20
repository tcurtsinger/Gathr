-- A room chat runs the small team model: messages, @names, replies, reactions, the board and the
-- lead's check, and none of the task machinery. Every chat made before this is an old one (0).
ALTER TABLE chats ADD COLUMN room INTEGER NOT NULL DEFAULT 0;
