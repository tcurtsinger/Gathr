-- The pair Gathr's look last judged a review for: "<commit reviewed>:<the PR's head>". The verdict
-- (`outdated`) is kept only for that pair, so a review recorded again, or a head that moves, is judged again.
ALTER TABLE tasks ADD COLUMN judged TEXT;
