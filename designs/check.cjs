const fs = require('fs'),
  path = require('path'),
  vm = require('vm'),
  assert = require('assert/strict');
const styles = ['shell', 'conversation', 'panels', 'responsive']
  .map(name => fs.readFileSync(path.join(__dirname, name + '.css'), 'utf8')).join('\n');
const pages = [['chat', 13], ['single-chat', 11], ['new-chat', 11], ['start-chat', 12]];
for (const [d, v] of pages) {
  const file = path.join(__dirname, d, 'v' + v + '.html'),
    h = fs.readFileSync(file, 'utf8');
  const ids = [...h.matchAll(/data-id="([^"]+)"/g)].map(x => x[1]);
  assert.equal(new Set(ids).size, ids.length, 'unique data IDs');
  assert.equal((h.match(/<h1/g) || []).length, 1);
  assert(!/fetch\(|Sample workspace|>↑<|>＋<|<span class="path">Gathr/.test(h), 'local content and polished icons');
  assert(h.includes('../tokens.css') && h.includes('../components.css'), 'shared styles');
  if (d === 'chat' || d === 'single-chat') assert.equal((h.match(/data-id="message-/g) || []).length, 42);
  for (const m of h.matchAll(/<script>([\s\S]*?)<\/script>/g)) new vm.Script(m[1]);
  for (const m of h.matchAll(/(?:href|location.href)=['"]([^'"#]+\.html)/g)) assert(fs.existsSync(path.resolve(path.dirname(file), m[1])), m[1]);
}
assert(styles.includes('.choose .agentcheck:checked'), 'picker styling');
console.log('Four selected pages: IDs, scripts, links, shared styles, fixtures and polish passed.');
assert(/\.chatrow\[hidden\]\s*\{\s*display:\s*none/.test(styles), 'search hides flex rows');
new vm.Script(fs.readFileSync(path.join(__dirname, 'navigation.js'), 'utf8'));
new vm.Script(fs.readFileSync(path.join(__dirname, 'phase1-prototype.js'), 'utf8'));
new vm.Script(fs.readFileSync(path.join(__dirname, 'settings-overlay.js'), 'utf8'));
for (const folder of ['diagnostics', 'engines', 'chat-states']) {
  for (const version of folder === 'engines' ? [1, 2, 3, 4] : [1, 2, 3]) {
    const html = fs.readFileSync(path.join(__dirname, folder, `v${version}.html`), 'utf8');
    const ids = [...html.matchAll(/data-id="([^"]+)"/g)].map(match => match[1]);
    assert.equal(new Set(ids).size, ids.length, folder + ': unique IDs');
    assert.equal((html.match(/<h1/g) || []).length, 1);
    assert(html.includes('../tokens.css') && html.includes('../components.css'), 'shared styles');
  }
}
console.log('Seven Phase 1 options: shared styles, IDs and script syntax passed.');
assert(/background:\s*var\(--backdrop\)/.test(styles), 'shared backdrop');
for (const [d, v] of pages) {
  const h = fs.readFileSync(path.join(__dirname, d, 'v' + v + '.html'), 'utf8');
  assert(h.includes('src="../navigation.js"') && h.includes('data-id="sidebar-reveal"'), 'shared navigation reveal');
}

new vm.Script(fs.readFileSync(path.join(__dirname, 'phase1-selected.js'), 'utf8'));
