-- A PR's size as Gathr measured it, its question, and once landed what git can't give (part G, 0054).
-- No foreign keys: a landed row outlives its chat and agents.
CREATE TABLE changes (
    project_id TEXT NOT NULL, pr INTEGER NOT NULL, chat_id TEXT NOT NULL,
    head TEXT NOT NULL, counted INTEGER NOT NULL, removed INTEGER NOT NULL, top TEXT NOT NULL,
    line INTEGER NOT NULL, measured_ts INTEGER NOT NULL,
    question INTEGER, asked INTEGER, let_through INTEGER,
    sent_back_head TEXT, sent_back_task INTEGER,
    builder_id TEXT, builder_engine TEXT, builder_model TEXT,
    reviewer_id TEXT, reviewer_engine TEXT, reviewer_model TEXT,
    unasked INTEGER, first_reviewed TEXT, by_gathr INTEGER, landed_ts INTEGER,
    PRIMARY KEY (project_id, pr)
) STRICT;
-- The commit a review's first report named: the record reads what came after it.
ALTER TABLE tasks ADD COLUMN first_reviewed TEXT;
