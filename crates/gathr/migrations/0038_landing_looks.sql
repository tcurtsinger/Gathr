-- What Gathr's last look at a PR it may land saw (0050), for whoever asks what the chat waits on:
-- the words, whether the PR moves by itself from here (its checks run, it is being brought up to
-- date) or is left to its reviewer, and when it looked.
ALTER TABLE landings ADD COLUMN saw TEXT;
ALTER TABLE landings ADD COLUMN moving INTEGER NOT NULL DEFAULT 1;
ALTER TABLE landings ADD COLUMN looked_ts INTEGER;
