-- A table or a chart an agent showed with `show`: a message of kind 'block', its body JSON
-- `{kind, title?, table | chart}`. SQLite changes a CHECK only by rebuilding the table, as 0028 did.
--
-- The search index points at messages by rowid (0015), so each message keeps its rowid and every
-- column, and the index is refilled from the text rows only: a block is never indexed. `reactions`
-- points at `messages` by name and finds the new one (foreign keys are off while migrating).
CREATE TABLE messages_next (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    turn_id TEXT,
    ts INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'gathr')),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'tool', 'thinking', 'block')),
    body TEXT NOT NULL,
    to_agent TEXT,
    copy_of TEXT,
    picks TEXT,
    reply_to TEXT
) STRICT;
INSERT INTO messages_next (rowid, id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of, picks, reply_to)
    SELECT rowid, id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of, picks, reply_to FROM messages;
-- Dropping the table drops its indexes and the three search triggers with it.
DROP TABLE messages;
ALTER TABLE messages_next RENAME TO messages;
CREATE INDEX messages_by_chat ON messages (chat_id, ts);
CREATE INDEX messages_person ON messages (chat_id, ts) WHERE role = 'user';
CREATE INDEX messages_opening ON messages (turn_id, ts) WHERE role IN ('user', 'gathr');

-- With every rowid kept the index already matches; the refill stays so it never rests on that (0015).
INSERT INTO message_search (message_search) VALUES ('delete-all');
INSERT INTO message_search (rowid, body) SELECT rowid, body FROM messages WHERE kind = 'text';

CREATE TRIGGER message_search_insert AFTER INSERT ON messages WHEN new.kind = 'text' BEGIN
    INSERT INTO message_search (rowid, body) VALUES (new.rowid, new.body);
END;

CREATE TRIGGER message_search_delete AFTER DELETE ON messages WHEN old.kind = 'text' BEGIN
    INSERT INTO message_search (message_search, rowid, body) VALUES ('delete', old.rowid, old.body);
END;

CREATE TRIGGER message_search_update AFTER UPDATE OF body, kind ON messages BEGIN
    INSERT INTO message_search (message_search, rowid, body)
        SELECT 'delete', old.rowid, old.body WHERE old.kind = 'text';
    INSERT INTO message_search (rowid, body) SELECT new.rowid, new.body WHERE new.kind = 'text';
END;
