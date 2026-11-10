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
console.log('scrub-frames: ok');
