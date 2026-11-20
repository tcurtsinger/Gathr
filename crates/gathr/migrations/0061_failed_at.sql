-- The commit whose CI had failed when a CI wait was met. On a turn, the failed CI it opens with: a
-- JSON list of {commit, repo, cwd}, each the commit that failed, the repository its wait named (or
-- none) and the folder its looks ran in, from a fired wait or a PR Gathr can't land on red CI. The
-- failed jobs' last lines are read there for that commit when the turn starts, not for a PR's head
-- by then.
ALTER TABLE waits ADD COLUMN failed_at TEXT;
ALTER TABLE turns ADD COLUMN failed_ci TEXT;
