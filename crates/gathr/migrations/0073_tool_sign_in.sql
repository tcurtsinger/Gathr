-- A tool that signs in through the browser: its sign-in (the tokens, and the client Gathr registered as), sealed
-- like a key, or a marker for what the Keychain keeps on a Mac. Never its text.
ALTER TABLE tools ADD COLUMN sign_in BLOB;
