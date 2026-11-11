-- A project's checks and merge settings (0036): whose result gates a merge (GitHub's CI, Gathr's
-- checks, or both) and how Gathr merges. Projects made before this use Gathr's checks: a new one
-- in a repo with GitHub Actions uses CI.
ALTER TABLE projects ADD COLUMN ci TEXT NOT NULL DEFAULT 'gathr' CHECK (ci IN ('ci', 'gathr', 'both'));
ALTER TABLE projects ADD COLUMN merge_method TEXT NOT NULL DEFAULT 'fast-forward'
    CHECK (merge_method IN ('fast-forward', 'merge', 'squash'));

-- The commands Gathr runs on a PR's head, in order. The person never types one (owner, task
-- 1455): the Supervisor sets them, or Gathr found them. `source` is the agent's id, or 'found'.
CREATE TABLE project_checks (
    project_id TEXT NOT NULL REFERENCES projects (id),
    position INTEGER NOT NULL,
    command TEXT NOT NULL,
    source TEXT NOT NULL,
    set_ts INTEGER NOT NULL,
    PRIMARY KEY (project_id, position)
) STRICT;
