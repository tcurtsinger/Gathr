-- Where the agent chose its turn's words show, with `post` (0079): `main`, `m:<message>` or `t:<task>`, the
-- vocabulary of `messages.about` (0055). Old turns have none and are placed by the default rule. A column
-- only: no row is rewritten, so every rowid and the search index stay.
ALTER TABLE turns ADD COLUMN placed TEXT;
