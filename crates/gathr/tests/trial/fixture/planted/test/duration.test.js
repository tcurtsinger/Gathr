const test = require('node:test');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { parseDuration } = require('../src/duration');

test('reads each unit', () => {
  assert.equal(parseDuration('2h'), 7200000);
  assert.equal(parseDuration('3m'), 180000);
  assert.equal(parseDuration('4s'), 4000);
  assert.equal(parseDuration('250ms'), 250);
});

test('reads parts together', () => {
  assert.equal(parseDuration('1h30m'), 5400000);
  assert.equal(parseDuration('1m30s500ms'), 90500);
});

test('is NaN for anything else', () => {
  for (const text of ['', '1x', 'h', '1.5h', '1h 30m']) {
    assert.ok(Number.isNaN(parseDuration(text)), text);
  }
});

test('the command line prints the milliseconds', () => {
  const out = execFileSync(process.execPath, ['src/cli.js', '1h30m'], { encoding: 'utf8' });
  assert.equal(out.trim(), '5400000');
});

test('reads upper-case units', () => {
  assert.equal(parseDuration('1H30M'), 5400000);
  assert.equal(parseDuration('2H'), 7200000);
});
