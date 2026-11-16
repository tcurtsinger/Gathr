-- Stage 3, part I (0063): building waits until a new ask's plan is approved. A look-into task only
-- looks into something, and is held for its whole life.
ALTER TABLE tasks ADD COLUMN look_into INTEGER NOT NULL DEFAULT 0 CHECK (look_into IN (0, 1));
-- When Gathr first refused or caught something of the ask: an unsized ask's size line shows from then.
ALTER TABLE asks ADD COLUMN held_ts INTEGER;
