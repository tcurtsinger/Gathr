-- Gathr runs a project's checks at a commit (0036 C3): each run, its steps, and when the
-- project's caches were last used, so caches unused for 14 days go.
CREATE TABLE check_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id),
    head TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('running', 'passed', 'failed')),
    why TEXT,
    trace_id TEXT NOT NULL,
    started_ts INTEGER NOT NULL,
    ended_ts INTEGER
) STRICT;

CREATE INDEX check_runs_by_head ON check_runs (project_id, head, started_ts);

CREATE TABLE check_steps (
    run_id TEXT NOT NULL REFERENCES check_runs (id),
    position INTEGER NOT NULL,
    command TEXT NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER NOT NULL,
    -- The end of what it printed, stdout then stderr.
    log TEXT NOT NULL,
    PRIMARY KEY (run_id, position)
) STRICT;

CREATE TABLE check_caches (
    project_id TEXT PRIMARY KEY REFERENCES projects (id),
    used_ts INTEGER NOT NULL
) STRICT;
