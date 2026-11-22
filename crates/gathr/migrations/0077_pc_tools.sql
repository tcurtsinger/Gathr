-- Tools on this PC: a program Gathr starts itself, with `link` empty. The program as typed, its arguments as a
-- JSON array, the folder it runs in when one is given, and its variables as one JSON map sealed like a key (or a
-- marker for what the Keychain keeps on a Mac): never their text.
ALTER TABLE tools ADD COLUMN program TEXT;
ALTER TABLE tools ADD COLUMN args TEXT;
ALTER TABLE tools ADD COLUMN folder TEXT;
ALTER TABLE tools ADD COLUMN variables BLOB;
