-- A task given in passing doesn't hold the task whose turn gave it, and is heard back on by itself.
ALTER TABLE tasks ADD COLUMN in_passing INTEGER NOT NULL DEFAULT 0 CHECK (in_passing IN (0, 1));
-- What its giver last heard of it (its state and reason then): an outcome that changes after that
-- is heard once more. Null for a task heard before this column, which is heard again only as before.
ALTER TABLE tasks ADD COLUMN heard_as TEXT;
