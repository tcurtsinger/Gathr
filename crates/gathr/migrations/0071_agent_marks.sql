-- An agent's picture is a shape, a pair of eyes and a line colour, each kept by name once the person picks
-- one. None picked: the window works all three out from the agent's id. The old drawn face's seed, gender,
-- skin tone and background, and the uploaded picture, go.
ALTER TABLE agents ADD COLUMN shape TEXT;
ALTER TABLE agents ADD COLUMN eyes TEXT;
ALTER TABLE agents ADD COLUMN colour TEXT;

ALTER TABLE agents DROP COLUMN face_seed;
ALTER TABLE agents DROP COLUMN gender;
ALTER TABLE agents DROP COLUMN skin;
ALTER TABLE agents DROP COLUMN background;
ALTER TABLE agents DROP COLUMN picture;
