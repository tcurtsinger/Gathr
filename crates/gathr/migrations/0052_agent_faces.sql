-- An agent's face (0074): the seed it is drawn from (its name when it was made, a new one at each
-- Shuffle, never changed by a rename), and the gender, skin tone and background the person set, if any.
-- A background is #RRGGBB in upper case; none is a grey until the person picks. The window draws the
-- face; nothing drawn is kept here. An agent from before starts from its name.
ALTER TABLE agents ADD COLUMN face_seed TEXT;
ALTER TABLE agents ADD COLUMN gender TEXT CHECK (gender IN ('man', 'woman'));
ALTER TABLE agents ADD COLUMN skin INTEGER CHECK (skin BETWEEN 1 AND 6);
ALTER TABLE agents ADD COLUMN background TEXT
  CHECK (background GLOB '#[0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F]');

UPDATE agents SET face_seed = name;
