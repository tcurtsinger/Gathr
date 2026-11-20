-- One agent can no longer be paused: its pauses go. The CHECK on `scope` still lists 'agent'; a
-- rebuild of the table only to drop it isn't worth it. A turn one held starts at the next start.
DELETE FROM pauses WHERE scope = 'agent';
