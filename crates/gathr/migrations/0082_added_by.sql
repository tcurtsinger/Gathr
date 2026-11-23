-- Who added a skill, a tool, an agent or a project, when an AI app outside Gathr did: the name the app gave. A skill is
-- a folder, so it is kept by its name; the rest by their id. The thing's own delete removes its row.
CREATE TABLE added_by (
    kind TEXT NOT NULL CHECK (kind IN ('skill', 'tool', 'agent', 'project')),
    id TEXT NOT NULL,
    app TEXT NOT NULL,
    added_ts INTEGER NOT NULL,
    PRIMARY KEY (kind, id)
) STRICT;
