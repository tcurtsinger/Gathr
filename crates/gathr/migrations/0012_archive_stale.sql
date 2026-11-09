-- Archive and stale (F11, docs/decisions/0010-coordination-as-state.md).

-- When the person archived the chat; NULL while it's in the list. Archiving ended its work.
ALTER TABLE chats ADD COLUMN archived_ts INTEGER;

-- A stale task's last move (a turn, message, wait or answer): nothing has moved it since, for the
-- stale days at least. NULL while it isn't stale; cleared when it moves.
ALTER TABLE tasks ADD COLUMN stale_ts INTEGER;

-- Why a queued or running turn is to stop: its task was cancelled. Kept on the turn, so a restart
-- that opens the task again never lets the old turn go on.
ALTER TABLE turns ADD COLUMN stop_why TEXT;
