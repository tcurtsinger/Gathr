-- A completed review remains evidence when a wait wakes its task back into working.
-- Transient task/turn state is not proof that its recorded commit was approved.
ALTER TABLE tasks ADD COLUMN review_done INTEGER NOT NULL DEFAULT 0 CHECK (review_done IN (0, 1));
UPDATE tasks SET review_done = 1
WHERE kind = 'review' AND reviewed IS NOT NULL
  AND (finish = 'done' OR state = 'done');
