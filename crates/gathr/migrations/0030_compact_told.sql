-- Whether an agent was told its conversation nears where it compacts (compaction plan, K3): once,
-- again only after a compaction or a failed turn.
ALTER TABLE agent_context ADD COLUMN told INTEGER NOT NULL DEFAULT 0;
-- The engine and model the window was seen on (`context::model_key`): another model's window never
-- sets where this one compacts.
ALTER TABLE agent_context ADD COLUMN window_of TEXT;
