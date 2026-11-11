-- The head commit of a task's PR as the PR looks last saw it, so the window shows the checks run
-- Gathr made at that commit on the task (0036 C6).
ALTER TABLE tasks ADD COLUMN pr_head TEXT;
