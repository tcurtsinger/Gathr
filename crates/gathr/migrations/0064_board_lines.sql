-- The board: a chat's pieces of work, each with its owner (an agent of the chat) and one of four states.
-- Agents set its lines with one tool; Gathr only shows them. Lines show in the order they were added, by id.
-- No foreign key to agents, as with reactions: a removed agent's lines stay.
CREATE TABLE board_lines (
    id INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats (id),
    owner TEXT NOT NULL,
    text TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('to_do', 'doing', 'in_review', 'done'))
) STRICT;
CREATE INDEX board_lines_by_chat ON board_lines (chat_id);
