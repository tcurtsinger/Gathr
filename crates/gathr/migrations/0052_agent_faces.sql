-- An agent's face (0074): the seed it is drawn from (its name when it was made, a new one at each
-- Shuffle, never changed by a rename), and the gender and skin tone the person set, if any. The window
-- draws it; nothing drawn is kept here. An agent from before starts from its name.
ALTER TABLE agents ADD COLUMN face_seed TEXT;
ALTER TABLE agents ADD COLUMN gender TEXT CHECK (gender IN ('man', 'woman'));
ALTER TABLE agents ADD COLUMN skin INTEGER CHECK (skin BETWEEN 1 AND 6);

UPDATE agents SET face_seed = name;
