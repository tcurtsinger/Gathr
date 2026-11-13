-- Friction (promise 11, part 11.4; 0048): what got in an agent's way, as it reported it with
-- `report_friction`. It points at no other table, so a chat or an agent that is deleted leaves the report.
CREATE TABLE friction (
    id INTEGER PRIMARY KEY,
    ts INTEGER NOT NULL,
    chat_id TEXT NOT NULL,
    agent_id TEXT,
    turn_id TEXT,
    task INTEGER,
    what TEXT NOT NULL,
    place TEXT
) STRICT;
