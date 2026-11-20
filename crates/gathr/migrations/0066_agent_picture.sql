-- A picture the person gave an agent, as the window sent it: a data URL of a small square image. None draws
-- the face from its seed. Removing the agent clears it.
ALTER TABLE agents ADD COLUMN picture TEXT;
