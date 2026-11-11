-- Finished PR cleanup survives service restarts and retries skipped work without deleting a
-- reused branch: its tip must still be the head GitHub reported as merged or closed.
CREATE TABLE pr_cleanup (
    project TEXT NOT NULL,
    branch TEXT NOT NULL,
    head TEXT NOT NULL,
    chat TEXT NOT NULL,
    trace TEXT NOT NULL,
    queued_ts INTEGER NOT NULL,
    skipped TEXT,
    PRIMARY KEY (project, branch)
) STRICT;
