-- The person's messages an AI app outside Gathr sent with `send`: the name the app gave as it connected. A chat's
-- delete removes its rows.
CREATE TABLE from_app (
    message_id TEXT PRIMARY KEY REFERENCES messages (id),
    app TEXT NOT NULL
) STRICT;
