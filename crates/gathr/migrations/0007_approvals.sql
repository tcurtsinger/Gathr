-- What an agent may do without asking (0029). Every agent, old and new, starts on auto: the
-- engine's own check runs each action, and Gathr still asks before a push or a write outside the
-- agent's worktree.
ALTER TABLE agents ADD COLUMN approvals TEXT NOT NULL DEFAULT 'auto'
    CHECK (approvals IN ('ask', 'edits', 'auto', 'full'));
