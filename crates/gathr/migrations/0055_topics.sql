-- What each message is about (0075): the three facts the service can't read from what it kept before.
-- ADD COLUMN rewrites no row, so every rowid, and the search index that points at them (0015), stay as they are.
-- No foreign keys: deleting a chat removes its messages and turns before the rows that name them.
-- The ask whose closing hear-back this turn is: the ask's result (0051).
ALTER TABLE turns ADD COLUMN result_of TEXT;
-- The reviewer's turn Gathr queued for a PR it can't land at this head.
ALTER TABLE landings ADD COLUMN turn TEXT;
-- What a line Gathr said in no turn is about: `t:<task>`, `m:<message>` (an ask, or a plan version's message),
-- or `main` for a line that is a main message of its own. Null for every other message, and for a line from before.
-- A later migration that rebuilds `messages` must carry this column, as 0028 carried `to_agent` and `copy_of`.
ALTER TABLE messages ADD COLUMN about TEXT;
-- What a live event's topic reads, one row at a time, so its cost never grows with the chat: a turn's
-- first message of the person's or Gathr's, the question whose answer a turn heard, a PR's tasks.
-- Indexes only: no row is rewritten.
CREATE INDEX messages_opening ON messages (turn_id, ts) WHERE role IN ('user', 'gathr');
CREATE INDEX tasks_heard ON tasks (heard_in) WHERE heard_in IS NOT NULL;
CREATE INDEX tasks_by_pr ON tasks (chat_id, pr) WHERE pr IS NOT NULL;
