-- The model the engine named for the agent's calls when the window was seen. An agent on a Latest
-- or its engine's default gets its compaction point from the window of the model it last ran; a
-- window seen on another model never sets it.
ALTER TABLE agent_context ADD COLUMN ran TEXT;
