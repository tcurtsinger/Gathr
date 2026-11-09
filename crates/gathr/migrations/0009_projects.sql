-- Projects as saved records (F8b): a name, a folder and a team that a new chat copies. The chat's own
-- folder and members are what count after that (0028).
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- Names are unique ignoring case, as accounts' are.
    name_key TEXT NOT NULL UNIQUE,
    folder TEXT NOT NULL,
    created_ts INTEGER NOT NULL
) STRICT;

CREATE TABLE project_members (
    project_id TEXT NOT NULL REFERENCES projects (id),
    agent_id TEXT NOT NULL REFERENCES agents (id),
    role TEXT NOT NULL CHECK (role IN ('supervisor', 'reviewer', 'member')),
    position INTEGER NOT NULL,
    PRIMARY KEY (project_id, agent_id)
) STRICT;

ALTER TABLE chats ADD COLUMN project_id TEXT REFERENCES projects (id);

-- The turn this one answers, for the inline thread: a task's turn and a hear-back answer the turn that
-- gave the task, a wake-up the turn that set the wait.
ALTER TABLE turns ADD COLUMN answers TEXT;
