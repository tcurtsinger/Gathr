-- Where the person left off (0077): per thread of a chat, the newest message they have seen there, and their own
-- fold or open of it. A new table: no other table changes, so every rowid and the search index stay as they are.
-- `topic` is the thread's key as history names it (`m:<message>`, `t:<task>`, `r:<turn>`, `b:<turn>`), or '' for
-- the chat's main line. Everything at or before `seen_ts` (µs) there counts as seen; the service never moves it back.
-- `folded` is the person's own fold (1) or open (0), or null for Gathr's rule. Deleting the chat removes its rows.
CREATE TABLE reads (
    chat_id TEXT NOT NULL REFERENCES chats(id),
    topic TEXT NOT NULL,
    seen_ts INTEGER NOT NULL DEFAULT 0,
    folded INTEGER,
    PRIMARY KEY (chat_id, topic)
) WITHOUT ROWID;
