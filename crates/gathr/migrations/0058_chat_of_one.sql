-- Every chat keeps its list of agents, a chat of one too, its agent as its lead (0028, 2026-11-18).
-- A chat with turns and no list gets one row: the agent of its latest turn, leaving out a removed one
-- (the agent a message there went to), as its Supervisor since its first turn. A chat whose turns ran
-- as several agents gets only that one. A chat with a list already, with no turns, or whose turns ran
-- as no agent is left as it is. Rows only: no table changes, so every rowid and the search index stay.
INSERT INTO chat_members (chat_id, agent_id, role, added_ts)
SELECT c.id, latest.agent_id, 'supervisor',
       (SELECT min(f.created_ts) FROM turns f WHERE f.chat_id = c.id)
FROM chats c
JOIN (SELECT t.chat_id, t.agent_id,
             row_number() OVER (PARTITION BY t.chat_id ORDER BY t.created_ts DESC, t.rowid DESC) AS n
      FROM turns t JOIN agents a ON a.id = t.agent_id
      WHERE a.removed_ts IS NULL) latest ON latest.chat_id = c.id AND latest.n = 1
WHERE NOT EXISTS (SELECT 1 FROM chat_members m WHERE m.chat_id = c.id);
