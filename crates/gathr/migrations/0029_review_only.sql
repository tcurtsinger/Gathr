-- Required checks `none`: a clean review alone gates a merge (0045, owner's room tasks 167-169).
-- SQLite can't change a CHECK, so the table is made again with every row.
CREATE TABLE projects_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- Names are unique ignoring case, as accounts' are.
    name_key TEXT NOT NULL UNIQUE,
    folder TEXT NOT NULL,
    created_ts INTEGER NOT NULL,
    folder_name TEXT,
    ci TEXT NOT NULL DEFAULT 'gathr' CHECK (ci IN ('ci', 'gathr', 'both', 'none')),
    merge_method TEXT NOT NULL DEFAULT 'fast-forward' CHECK (merge_method IN ('fast-forward', 'merge', 'squash')),
    ci_source TEXT,
    ci_set_ts INTEGER,
    merge_source TEXT,
    merge_set_ts INTEGER
) STRICT;

INSERT INTO projects_next (id, name, name_key, folder, created_ts, folder_name, ci, merge_method, ci_source,
                           ci_set_ts, merge_source, merge_set_ts)
SELECT id, name, name_key, folder, created_ts, folder_name, ci, merge_method, ci_source, ci_set_ts,
       merge_source, merge_set_ts
FROM projects;

DROP TABLE projects;
ALTER TABLE projects_next RENAME TO projects;
