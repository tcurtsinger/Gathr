-- What a wait's last look saw while it isn't met ("1 of 2 checks finished: 1 passed; queued: UI
-- (macOS)"), for whoever asks what the chat waits on. `looked_ts` already says when.
ALTER TABLE waits ADD COLUMN seen TEXT;
