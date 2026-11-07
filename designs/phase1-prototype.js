const screen = document.body.dataset.screen;
const option = Number(document.body.dataset.option);
const surface = document.querySelector('#surface');
const light = location.hash.includes('light');
document.documentElement.classList.toggle('light', light);
const theme = document.querySelector('#theme');
theme.textContent = light ? 'Dark' : 'Light';
theme.onclick = () => {
  const next = document.documentElement.classList.toggle('light');
  history.replaceState(null, '', next ? '#light' : location.pathname);
  theme.textContent = next ? 'Dark' : 'Light';
};
function notify(text) { document.querySelector('.toast').textContent = text; }
function search(value) {
  document.querySelectorAll('.chatrow').forEach(row => { row.hidden = !row.textContent.toLowerCase().includes(value.toLowerCase()); });
}
document.querySelector('[data-id="nav-new"]').onclick = () => { location.href = '../start-chat/v12.html'; };
document.querySelector('[data-id="nav-search"]').onclick = () => {
  const input = document.querySelector('.search');
  input.hidden = !input.hidden;
  if (!input.hidden) input.focus();
};
document.querySelectorAll('.profile button').forEach(button => { button.onclick = () => { location.href = '../engines/v1.html'; }; });
const ref = '7ac91e02';
const sendRef = '82b940ef';
const topics = ['Reconnect streaming', 'Preserve the draft', 'Read reconnect tests', 'Check the event order', 'Handle a closed socket', 'Review the patch'];
const turns = Array.from({length: 48}, (_, i) => ({
  id: 48 - i, topic: topics[i % topics.length], end: i === 0 || i === 2 ? 'Failed' : i === 1 ? 'Stopped' : 'Completed',
  tools: i === 0 ? 50 : i === 2 ? 0 : 2 + i % 8,
  duration: i === 0 ? '2m 14s' : i === 2 ? '18 ms' : `${12 + (2 + i % 8) * 2.5}s`
}));
function failure(turn) {
  return `<div class="notice" data-id="failure-notice-${turn.id}"><strong>Tool call failed</strong>
    <button class="ref" data-id="copy-reference-${turn.id}" title="Copy reference">${ref}</button></div>
    <p>Read failed: file not found, src/reconnect-policy.ts.</p>`;
}
function events(turn) {
  if (turn.id === 46) return `<div class="facts"><span>Ended<b>Failed</b></span><span>Duration<b>18 ms</b></span>
    <span>Tokens<b>Not reported</b></span></div><div class="notice">Send failed <button class="ref">${sendRef}</button></div>
    <p>Engine start failed. File not found: C:\\missing\\claude.exe.</p>
    <div class="event"><time>+0 ms</time><span>Message queued</span><small></small></div>
    <div class="event"><time>+18 ms</time><span>Engine start failed</span><small>18 ms</small></div>`;
  const failed = turn.end === 'Failed';
  const tools = Array.from({length: turn.tools}, (_, i) => {
    const last = failed && i === turn.tools - 1;
    const name = ['Read', 'Search', 'Read', 'Shell', 'Edit'][i % 5];
    return `<div class="event" data-id="tool-${turn.id}-${i}"><time>+${(2 + i * 2.5).toFixed(1)}s</time>
      <span>${last ? 'Read' : name} <span class="statusword">${last ? 'Failed' : 'Completed'}</span></span>
      <small>${last ? '12 ms' : `${80 + i * 21} ms`}</small><details><summary>Details</summary>
      <pre>${last ? 'src/reconnect-policy.ts\nFile not found' : `src/reconnect-${i + 1}.ts\nResult recorded`}</pre></details></div>`;
  }).join('');
  return `<div class="facts" data-id="turn-facts-${turn.id}">
    <span>Ended<b>${turn.end}</b></span><span>Duration<b>${turn.duration}</b></span>
    <span>Input tokens<b>18,420</b></span><span>Output tokens<b>1,204</b></span>
    <span>Cache read<b>12,000</b></span><span>Cache write<b>0</b></span></div>
    <details><summary>Gathr context estimates</summary>
    <p>Briefing: 2,400 characters, about 600 tokens. Ask: 140 characters, about 35 tokens.</p>
    <p>Provider totals above. Conversation and engine context are not split.</p></details>
    ${failed ? failure(turn) : ''}
    <div class="event"><time>+0.0s</time><span>Model call 1</span><small>1.8 s</small></div>
    <details class="tools" data-id="tools-${turn.id}"><summary>${turn.tools} tool calls</summary>${tools}</details>
    <div class="event"><time>+${failed ? 128 : 6 + turn.tools * 2.5}s</time><span>Model call 2</span><small>6.0 s</small></div>
    <div class="event"><time>End</time><span>Turn ${turn.end.toLowerCase()}</span><small>${failed ? ref : ''}</small></div>`;
}
function copyRefs() {
  document.querySelectorAll('.ref').forEach(button => {
    // Clipboard access is intentionally absent in this prototype on the owner's PC.
    button.onclick = () => notify('Copy preview: ' + button.textContent.trim());
  });
}
function diagnostics() {
  surface.innerHTML = `<div class="pagehead"><h2>Reconnect streaming</h2><div class="toolbar">
    <label>Turns <select id="filter"><option>All</option><option>Failed</option><option>Stopped</option></select></label>
    <label>Find <input id="find" placeholder="Turn or reference"></label><span class="statusword" id="count">48 turns</span>
    </div></div><div id="diagnostic-content"></div>`;
  function render() {
    const filter = document.querySelector('#filter').value;
    const find = document.querySelector('#find').value.toLowerCase();
    const shown = turns.filter(t => (filter === 'All' || t.end === filter)
      && `${t.id} ${t.topic} ${t.end === 'Failed' ? t.id === 46 ? sendRef : ref : ''}`.toLowerCase().includes(find));
    document.querySelector('#count').textContent = `${shown.length} turns`;
    const target = document.querySelector('#diagnostic-content');
    if (!shown.length) { target.innerHTML = '<p class="empty">No matching turns</p>'; return; }
    if (option === 1) {
      target.innerHTML = `<div class="split"><nav class="turnlist" aria-label="Turns">${shown.map((t, i) =>
        `<button class="turnpick" data-id="turn-${t.id}" data-turn="${t.id}" aria-pressed="${i === 0}">
        Turn ${t.id}: ${t.end}<span>${t.topic}</span><span>${t.tools} tools, ${t.duration}</span></button>`).join('')}
        </nav><section class="trace" id="trace" aria-label="Selected turn"></section></div>`;
      const select = t => {
        document.querySelector('#trace').innerHTML = `<h2>Turn ${t.id}</h2>${events(t)}`;
        document.querySelectorAll('.turnpick').forEach(b => b.setAttribute('aria-pressed', b.dataset.turn === String(t.id)));
        copyRefs();
      };
      document.querySelectorAll('.turnpick').forEach(b => { b.onclick = () => select(turns.find(t => t.id === Number(b.dataset.turn))); });
      select(shown[0]);
    } else {
      target.innerHTML = `<div class="chronicle">${shown.map((t, i) => `<details class="turnblock" ${i === 0 ? 'open' : ''}>
        <summary data-id="turn-${t.id}">Turn ${t.id}: ${t.end}<span>${t.topic}</span></summary>${events(t)}</details>`).join('')}</div>`;
      copyRefs();
    }
  }
  document.querySelector('#filter').onchange = render;
  document.querySelector('#find').oninput = render;
  document.querySelector('#find').value = new URLSearchParams(location.search).get('ref') || '';
  render();
}
let login = 'Signed out';
function engines() {
  surface.innerHTML = `${option === 2 ? '<div class="engine-layout"><nav class="engine-menu"><button>Claude Code<small>Signed out</small></button></nav>' : ''}
    <section class="settings"><h2>Engines</h2><div class="preview-state"><label for="engine-preview">Preview state</label>
    <select id="engine-preview"><option>Signed out</option><option>Signed in</option><option>Path missing</option></select></div>
    <div class="settingrow"><h3>Claude Code</h3><span id="engine-state" class="statusword">Signed out</span>
    <button class="primary" id="sign-in">Sign in</button></div>
    <div class="pathfield"><label for="engine-path">Claude Code path</label><div class="pathline">
    <input id="engine-path" value="C:\\Tools\\Claude\\claude.exe" spellcheck="false" aria-describedby="path-error">
    <button id="save-path">Save</button></div><p id="path-error" class="field-error" hidden>Engine check failed. File not found.
    <button class="ref">${sendRef}</button></p></div>
    <div class="settingrow"><span>Login folder</span><details><summary>Show</summary><code>&lt;Gathr data&gt;/accounts/primary</code></details></div>
    </section>${option === 2 ? '</div>' : ''}`;
  function state() {
    document.querySelector('#engine-state').textContent = login;
    document.querySelector('#sign-in').textContent = login === 'Signed in' ? 'Sign out' : 'Sign in';
    const small = document.querySelector('.engine-menu small');
    if (small) small.textContent = login;
  }
  document.querySelector('#sign-in').onclick = () => {
    if (login === 'Signed in') { login = 'Signed out'; state(); }
    else document.querySelector('#signin').showModal();
  };
  document.querySelector('#complete-signin').onclick = () => {
    login = 'Signed in'; state(); document.querySelector('#signin').close(); notify('Signed in, preview');
  };
  document.querySelector('#engine-preview').onchange = e => {
    const missing = e.target.value === 'Path missing';
    login = e.target.value === 'Signed in' ? 'Signed in' : 'Signed out';
    document.querySelector('#engine-path').value = missing ? 'C:\\missing\\claude.exe' : 'C:\\Tools\\Claude\\claude.exe';
    document.querySelector('#path-error').hidden = !missing;
    document.querySelector('#engine-path').setAttribute('aria-invalid', String(missing)); state();
  };
  document.querySelector('#save-path').onclick = () => notify('Saved, preview only');
  copyRefs();
}
function chat() {
  surface.innerHTML = `<div class="chat-stage"><div class="statebar"><label for="chat-preview">Preview state</label>
    <select id="chat-preview"><option>Stopped</option><option>Waiting for sign-in</option><option>Error</option></select></div>
    <div class="transcript"><div class="reading" id="messages"></div></div>
    <div class="composerarea"><div class="reading"><div id="composer-notice"></div>
    <div class="composer"><textarea aria-label="Message" placeholder="Message"></textarea>
    <div class="composerbottom"><button class="primary" id="send">Send</button></div></div></div></div></div>`;
  const messages = document.querySelector('#messages');
  const leadin = `<article class="message user"><div class="who">You</div>
    <div class="content"><p>Check the reconnect path and keep the draft intact.</p></div></article>`;
  function render() {
    const value = document.querySelector('#chat-preview').value;
    const notice = value === 'Stopped'
      ? '<span>Stopped</span><button id="continue">Continue</button>'
      : value === 'Waiting for sign-in'
        ? '<span>Waiting for sign-in</span><a href="../engines/v1.html">Sign in</a>'
        : `<span>Send failed</span><button class="ref">${sendRef}</button><a href="../diagnostics/v1.html?ref=${sendRef}">Diagnostics</a>`;
    const box = `<div class="notice" data-id="chat-notice" role="status">${notice}</div>`;
    messages.innerHTML = leadin + (value === 'Stopped' ? `<article class="message"><div class="who">Assistant</div>
      <details><summary>12 tool calls</summary><p>Read reconnect tests, search handlers, inspect draft storage.</p></details>
      <p>The draft is saved before reconnect starts. I found a second path that clears it when the socket closes.</p></article>` : '');
    document.querySelector('#composer-notice').innerHTML = option === 2 ? box : '';
    if (option === 1) messages.insertAdjacentHTML('beforeend', box);
    const continuing = document.querySelector('#continue');
    if (continuing) continuing.onclick = () => {
      document.querySelector('textarea').value = 'Continue'; document.querySelector('textarea').focus();
    };
    document.querySelector('#send').disabled = value === 'Waiting for sign-in';
    copyRefs();
  }
  document.querySelector('#chat-preview').onchange = render;
  document.querySelector('#send').onclick = () => {
    const input = document.querySelector('textarea');
    if (!input.value.trim()) { input.focus(); return; }
    const article = document.createElement('article'); article.className = 'message user';
    const who = document.createElement('div'); who.className = 'who'; who.textContent = 'You';
    const p = document.createElement('p'); p.textContent = input.value; article.append(who, p); messages.append(article);
    input.value = ''; notify('Sent, preview only');
  };
  render();
}
if (screen === 'diagnostics') diagnostics();
if (screen === 'engines') engines();
if (screen === 'chat-states') chat();
