-- A review's notes (0053), at the commit reviewed. `fix`: the task Gathr gave for them.
CREATE TABLE review_notes (
    id INTEGER PRIMARY KEY,
    task INTEGER NOT NULL REFERENCES tasks (number),
    reviewed TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('wrong', 'unasked', 'suggestion')),
    file TEXT,
    says TEXT NOT NULL,
    fix INTEGER REFERENCES tasks (number),
    created_ts INTEGER NOT NULL
) STRICT;
CREATE INDEX review_notes_task ON review_notes (task);
