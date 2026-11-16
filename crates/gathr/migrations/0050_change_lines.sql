-- Stage 3, part J (0064): the size of the ask a landed change answered ('repair', 'new' or none),
-- which a Fix's line reads, and which line a change was measured against ('project', 'fix', 'part').
ALTER TABLE changes ADD COLUMN size TEXT;
ALTER TABLE changes ADD COLUMN line_of TEXT NOT NULL DEFAULT 'project';
