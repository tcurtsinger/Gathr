-- Computer use (cua-driver) as a tool on this PC in Plugins, switched off: its switch is the one way to turn it
-- on. What it runs is Gathr's own fetched copy (`gathr::computer`), not the program named here.
INSERT OR IGNORE INTO tools (id, name, link, enabled, added_ts, program, args)
VALUES ('computer', 'Computer', '', 0, CAST(strftime('%s', 'now') AS INTEGER) * 1000000, 'cua-driver', '["mcp"]');
