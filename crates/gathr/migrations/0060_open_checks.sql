-- The open check (0080): once per message of the person's, when the lead has answered it and the chat
-- has gone quiet, every free member who took no part checks the answer in a turn of its own. A round is
-- the message it is for, the lead's message it checks and the turn that holds it, when it began, the
-- lead's last-word turn when a member wrote words, and when it ended. Its check turns are the turns
-- opened by `check` that answer `answer_turn`.
CREATE TABLE open_checks (
    id INTEGER PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats(id),
    asked TEXT NOT NULL,
    answer TEXT,
    answer_turn TEXT,
    began_ts INTEGER NOT NULL,
    last_word TEXT,
    ended_ts INTEGER,
    UNIQUE (chat_id, asked)
);

-- Every message of the person's sent before it counts as had: an upgrade starts no round for an old
-- message. A round with no answer is one of these. New rows only: no row is rewritten.
INSERT INTO open_checks (chat_id, asked, began_ts, ended_ts)
SELECT m.chat_id, m.id, m.ts, m.ts FROM messages m JOIN turns u ON u.id = m.turn_id
WHERE m.role = 'user' OR (m.role = 'gathr' AND u.opened_by = 'sent_back');
