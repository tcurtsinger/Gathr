-- Search over message text (plans/search.md): an FTS5 index of text messages only, word prefixes,
-- case and accents folded. External content: the text stays in `messages`, read by rowid.
--
-- Never run the 'rebuild' command on it: that indexes every row, tool JSON too. To refill it,
-- delete all, then insert the text rows as below.
--
-- A later migration that rebuilds `messages` must keep each rowid
-- (INSERT INTO messages_new (rowid, ...) SELECT rowid, ...), then refill this index: the index
-- points at messages by rowid, and tests/search.rs fails if they drift apart.
CREATE VIRTUAL TABLE message_search USING fts5(
    body,
    content = 'messages',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

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
