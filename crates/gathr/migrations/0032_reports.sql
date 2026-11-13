-- A task keeps its owner's last report (finish, finish_note): it is no longer wiped when a turn ends.
-- With it, what to look at besides the PR (a commit or a file, for work), and when it was made.
ALTER TABLE tasks ADD COLUMN finish_see TEXT;
ALTER TABLE tasks ADD COLUMN finish_ts INTEGER;
-- "left" no longer starts its owner again, so idle lefts aren't counted. The column now counts the
-- times Gathr asked the owner for a report since the last one; old counts would read as asks made.
ALTER TABLE tasks RENAME COLUMN idle_lefts TO asks;
UPDATE tasks SET asks = 0;
