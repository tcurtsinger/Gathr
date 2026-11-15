-- Which of Gathr's words opened each turn, and their version. Turns from before keep NULL
-- in both: not recorded. No CHECK on the kind: a new kind would rebuild `turns`.
ALTER TABLE turns ADD COLUMN opened_by TEXT;
ALTER TABLE turns ADD COLUMN opened_version INTEGER;
