-- Compaction belongs to a conversation, including while another task runs or the service stops.
ALTER TABLE task_sessions ADD COLUMN summed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE engine_sessions ADD COLUMN summed INTEGER NOT NULL DEFAULT 0;
