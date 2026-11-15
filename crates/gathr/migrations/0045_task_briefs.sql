-- A work task's complete brief: what done means, how it's proved, where the work lives, and how big
-- it should be. An agent can't give a work task without the first three; tasks from before have
-- none. NULL is "not given": reviews and the person's questions have none.
ALTER TABLE tasks ADD COLUMN done TEXT;
ALTER TABLE tasks ADD COLUMN proof TEXT;
ALTER TABLE tasks ADD COLUMN place TEXT;
ALTER TABLE tasks ADD COLUMN size TEXT;
ALTER TABLE tasks ADD COLUMN size_lines INTEGER;
-- When its first turn started: its giver may edit it until then.
ALTER TABLE tasks ADD COLUMN started_ts INTEGER;
-- The carry-on its queued brief was built with, so an edit rebuilds the same brief.
ALTER TABLE tasks ADD COLUMN carry TEXT;
-- Its giver's last edit: when, and the fields it changed, comma-separated.
ALTER TABLE tasks ADD COLUMN edited_ts INTEGER;
ALTER TABLE tasks ADD COLUMN edited TEXT;
-- A task that ran before this counts as started. When it started isn't kept, so its last change
-- stands in. Waiting, ready, blocked (by its `after`) and cancelled tasks may never have run: only a
-- turn on it that ran or ended says so.
UPDATE tasks SET started_ts = updated_ts
WHERE state IN ('working', 'held', 'done', 'failed', 'stopped')
   OR EXISTS (SELECT 1 FROM turns u WHERE u.task = tasks.number
              AND u.status IN ('running', 'done', 'failed', 'stopped'));
