-- Several questions an agent asks in one `ask_person` call, answered together: `parts` is a JSON array of
-- `{topic, question, choices}`, and once answered `answers` a JSON array of `{pick, words}` in the same order. Null on
-- a question asked alone, which keeps `choices`, `pick` and `words`.
ALTER TABLE questions ADD COLUMN parts TEXT;
ALTER TABLE questions ADD COLUMN answers TEXT;
