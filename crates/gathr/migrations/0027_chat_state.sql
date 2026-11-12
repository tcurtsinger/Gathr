-- The chat list says which chats work and which need the person (0040). Two small indexes, by chat:
-- the turns running or waiting for a sign-in, and the questions open to the person. The list then
-- reads no chat's turns or tasks, however long chats grow.
CREATE INDEX turns_live ON turns (chat_id, status) WHERE status IN ('running', 'waiting_sign_in');
CREATE INDEX tasks_asked ON tasks (chat_id) WHERE kind = 'person' AND state = 'ready';
