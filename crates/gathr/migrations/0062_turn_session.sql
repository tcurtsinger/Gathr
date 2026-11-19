-- The conversation each turn ran in: the session its engine reported at the turn's Init (0055). What
-- a turn hears since its conversation last ran (0084) counts only the turns that ran there, not those
-- of the same task that ran in another conversation, as a turn does under a background command (N4).
-- Turns from before have none.
ALTER TABLE turns ADD COLUMN session_id TEXT;

-- The question whose answer settled a request: what a turn that tells the request's answer carries.
ALTER TABLE tasks ADD COLUMN settled_by INTEGER;

-- The questions to the person whose answers a turn carries (a hear-back, or a brief readied by the
-- answer): its opening doesn't list them again (0084). `heard_in` stays the turn the answer was heard in.
CREATE TABLE turn_answers (
    turn_id TEXT NOT NULL REFERENCES turns (id) ON DELETE CASCADE,
    task INTEGER NOT NULL REFERENCES tasks (number) ON DELETE CASCADE,
    PRIMARY KEY (turn_id, task)
) STRICT, WITHOUT ROWID;
