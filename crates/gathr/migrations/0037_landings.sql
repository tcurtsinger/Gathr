-- Gathr lands a pull request (0049). A reviewer's sign-off is said with the review and kept with
-- it: it stands while that review covers the PR's head.
ALTER TABLE tasks ADD COLUMN signed_off INTEGER NOT NULL DEFAULT 0;

-- What Gathr did to land a chat's PR at one head, each at most once: asked GitHub to bring it up
-- to date, told its reviewer it can't land, pushed a merge GitHub didn't confirm. Kept so a look,
-- two triggers at once and a restart never do or say the same thing twice for one head.
CREATE TABLE landings (
    chat_id TEXT NOT NULL REFERENCES chats (id),
    pr INTEGER NOT NULL,
    head TEXT NOT NULL,
    updated_ts INTEGER,
    told_ts INTEGER,
    pushed_ts INTEGER,
    PRIMARY KEY (chat_id, pr, head)
);
