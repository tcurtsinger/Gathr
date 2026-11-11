-- Who set a project's CI choice and merge method, and when (Orin's task #88): its Supervisor
-- (the agent's id) through Gathr's tool, or `person` in Settings. NULL until someone sets it.
ALTER TABLE projects ADD COLUMN ci_source TEXT;
ALTER TABLE projects ADD COLUMN ci_set_ts INTEGER;
ALTER TABLE projects ADD COLUMN merge_source TEXT;
ALTER TABLE projects ADD COLUMN merge_set_ts INTEGER;
