-- The commit whose CI had failed when a CI wait was met, and on a turn the commits (space-separated)
-- whose failed CI it opens with: a fired wait's, or the head of a PR Gathr can't land. The failed
-- jobs' last lines are read for that commit when the turn starts, not for the PR's head by then.
ALTER TABLE waits ADD COLUMN failed_at TEXT;
ALTER TABLE turns ADD COLUMN failed_at TEXT;
