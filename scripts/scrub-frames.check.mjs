// Fails if scrub-frames.mjs lets a person's own names into a public recording.
// usage: node scripts/scrub-frames.check.mjs
import assert from 'node:assert/strict';
import { scrub } from './scrub-frames.mjs';

const init = scrub({
  type: 'system',
  subtype: 'init',
  agents: ['Explore', 'private-client-reviewer'],
  skills: ['private-skill'],
  plugins: [{ name: 'private-plugin', source: 'private-plugin@market' }, { name: 'cc-plugin-agents-md', source: 'cc-plugin-agents-md@builtin' }],
});
const text = JSON.stringify(init);
for (const name of ['private-client-reviewer', 'private-skill', 'private-plugin']) {
  assert.ok(!text.includes(name), `a name from outside Gathr survived the scrub: ${name} in ${text}`);
}
assert.equal(init.agents.length, 2, 'the scrub keeps how many sub-agents loaded');
assert.equal(init.agents[0], 'Explore', 'built-in sub-agents keep their names');
assert.equal(init.plugins[1].name, 'cc-plugin-agents-md', 'built-in plugins keep their names');
const remote = JSON.stringify(scrub({ method: 'remoteControl/status/changed', params: { serverName: 'PRIVATE-PC', installationId: 'abc' } }));
assert.ok(!remote.includes('PRIVATE-PC') && !remote.includes('abc'), `Codex's computer name survived the scrub: ${remote}`);
const account = JSON.stringify(scrub({ id: 2, result: { account: { type: 'chatgpt', email: 'private@mail.test' } } }));
assert.ok(!account.includes('private@mail.test'), `a Codex account's email survived the scrub: ${account}`);

const changed = JSON.stringify(scrub({ method: 'account/updated', params: { authMode: 'chatgpt', planType: 'private-plan' } }));
assert.ok(!changed.includes('private-plan'), 'Codex account/updated exposed the account plan');

// Exercise the public scrub command with forward, Windows and doubled Windows paths.
const { mkdtempSync, writeFileSync, readFileSync, rmSync } = await import('node:fs');
const { tmpdir } = await import('node:os');
const { join } = await import('node:path');
const { spawnSync } = await import('node:child_process');
const folder = mkdtempSync(join(tmpdir(), 'gathr-scrub-'));
try {
  const original = 'X:/private-person/test-data';
  const paths = [original, original.replaceAll('/', '\\'), original.replaceAll('/', '\\\\')];
  const source = join(folder, 'raw.jsonl');
  const result = join(folder, 'clean.jsonl');
  writeFileSync(source, JSON.stringify({ dir: 'out', line: JSON.stringify({ paths }) }) + '\n');
  const run = spawnSync(process.execPath, ['scripts/scrub-frames.mjs', source, result, original], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  const clean = JSON.parse(JSON.parse(readFileSync(result, 'utf8')).line);
  assert.deepEqual(clean.paths, ['gathr-test-data', 'gathr-test-data', 'gathr-test-data'], 'a Windows path survived the scrub');
} finally {
  rmSync(folder, { recursive: true });
}

console.log('scrub-frames: ok');
