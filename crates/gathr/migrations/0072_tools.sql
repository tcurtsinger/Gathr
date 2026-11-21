-- Tools that live on the web: an MCP server's link, added for every agent. Its key, when it has one, is
-- sealed for this computer's user (DPAPI on Windows) or kept in the Keychain on a Mac, where `key` holds
-- only a marker; never the key's text.
CREATE TABLE tools (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    link TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    key BLOB,
    added_ts INTEGER NOT NULL
);
