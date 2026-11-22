-- A schedule's short name, given with `schedule`. One made before has none: the service shows the first line of its
-- words instead.
ALTER TABLE schedules ADD COLUMN name TEXT;
