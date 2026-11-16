// T1's hidden test (0066): written into the builder's worktree after its diff is read, run with
// `node <this file>` from there, and deleted. It asks only what the brief asked.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(`${process.cwd()}/`);
const examples = [
  [5400000, '1h30m'],
  [90500, '1m30s500ms'],
  [250, '250ms'],
  [0, '0s'],
];

test('formatDuration gives the four examples', () => {
  const { formatDuration } = require('./src/duration.js');
  for (const [ms, text] of examples) assert.equal(formatDuration(ms), text);
});

test('--format prints them', () => {
  for (const [ms, text] of examples) {
    const out = execFileSync(process.execPath, ['src/cli.js', '--format', String(ms)], { encoding: 'utf8' });
    assert.equal(out.trim(), text);
  }
});
