-- The check commands the person allowed for a project (0036, owner's call on task 1455): a
-- command the project's own files don't name asks the person once, then never again there.
CREATE TABLE project_check_allowed (
    project_id TEXT NOT NULL REFERENCES projects (id),
    command TEXT NOT NULL,
    allowed_ts INTEGER NOT NULL,
    PRIMARY KEY (project_id, command)
) STRICT;
