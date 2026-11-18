-- The names the person picked from the window's "@" list, kept with their message (0028, 0072): a JSON list of
-- {at, len, agent}, in UTF-16 units of the body. Null for every other message, and for one saved before this.
-- ADD COLUMN rewrites no row, so every rowid, and the search index that points at them (0015), stay as they are.
-- A later migration that rebuilds `messages` must carry this column, as 0028 carried `to_agent` and `copy_of`.
ALTER TABLE messages ADD COLUMN picks TEXT;
