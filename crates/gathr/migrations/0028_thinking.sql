-- A turn shows that its agent thought, as a time only (0042): a message of kind 'thinking', its
-- body JSON `{status, duration_ms, tokens?}`. SQLite changes a CHECK only by rebuilding the table.
--
-- The search index points at messages by rowid (0015), so each message keeps its rowid, and the
-- index is refilled from the text rows: never by 'rebuild', which would index every row.
CREATE TABLE messages_new (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    turn_id TEXT,
    ts INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'gathr')),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'tool', 'thinking')),
    body TEXT NOT NULL,
    to_agent TEXT,
    copy_of TEXT
) STRICT;
INSERT INTO messages_new (rowid, id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of)
    SELECT rowid, id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of FROM messages;
-- Dropping the table drops its index and the three search triggers with it.
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;
CREATE INDEX messages_by_chat ON messages (chat_id, ts);

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
