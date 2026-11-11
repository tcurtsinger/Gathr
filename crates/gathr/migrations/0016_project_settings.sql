-- A project's checks and merge settings (0036): the commands Gathr runs on a PR's head, in order;
-- whose result gates a merge (GitHub's CI, Gathr's checks, or both); how Gathr merges.
-- Projects made before this use Gathr's checks: a new one in a repo with GitHub Actions uses CI.
ALTER TABLE projects ADD COLUMN checks TEXT NOT NULL DEFAULT '[]' CHECK (json_type(checks) = 'array');
ALTER TABLE projects ADD COLUMN ci TEXT NOT NULL DEFAULT 'gathr' CHECK (ci IN ('ci', 'gathr', 'both'));
ALTER TABLE projects ADD COLUMN merge_method TEXT NOT NULL DEFAULT 'fast-forward'
    CHECK (merge_method IN ('fast-forward', 'merge', 'squash'));
