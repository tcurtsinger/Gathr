-- The conversation each turn ran in: the session its engine reported at the turn's Init (0055). What
-- a turn hears since its conversation last ran (0084) counts only the turns that ran there, not those
-- of the same task that ran in another conversation, as a turn does under a background command (N4).
-- Turns from before have none.
ALTER TABLE turns ADD COLUMN session_id TEXT;
