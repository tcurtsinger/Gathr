-- The turn Gathr told a person task's giver it was settled in, so the chat shows the person's answer
-- there (ux-issues 26). NULL until then, and for tasks settled before this column.
ALTER TABLE tasks ADD COLUMN heard_in TEXT;
