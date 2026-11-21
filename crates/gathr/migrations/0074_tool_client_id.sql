-- The Client ID an admin made for Gathr, for a tool whose sign-in service takes no app it doesn't know: shown on
-- the tool's page, so not sealed. Its Client secret, when there is one, is sealed with the tool's sign-in.
ALTER TABLE tools ADD COLUMN client_id TEXT;
