-- The event record (docs/decisions/0011-event-record.md).
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    ts INTEGER NOT NULL,
    name TEXT NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('start', 'end', 'point')),
    trace_id TEXT,
    span_id TEXT,
    parent_span_id TEXT,
    room_id TEXT,
    conversation_id TEXT,
    agent_id TEXT,
    task_id TEXT,
    duration_us INTEGER,
    status TEXT CHECK (status IN ('ok', 'error')),
    error_type TEXT,
    attrs BLOB
) STRICT;

CREATE INDEX events_by_conversation ON events (conversation_id, ts);
CREATE INDEX events_by_room ON events (room_id, ts);
CREATE INDEX events_by_trace ON events (trace_id);
CREATE INDEX events_by_name ON events (name, ts);

-- attrs are JSONB; this view shows them as text, with a readable time and the error reference.
CREATE VIEW event_log AS
SELECT
    id,
    strftime('%Y-%m-%dT%H:%M:%fZ', ts / 1000000.0, 'unixepoch') AS time,
    ts,
    name,
    phase,
    substr(trace_id, 1, 8) AS ref,
    trace_id,
    span_id,
    parent_span_id,
    room_id,
    conversation_id,
    agent_id,
    task_id,
    duration_us,
    status,
    error_type,
    json(attrs) AS attrs
FROM events;

CREATE TABLE chats (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_ts INTEGER NOT NULL
) STRICT;

-- kind 'tool' rows hold a JSON body: the tool's name, status and duration.
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    turn_id TEXT,
    ts INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'tool')),
    body TEXT NOT NULL
) STRICT;

CREATE INDEX messages_by_chat ON messages (chat_id, ts);
