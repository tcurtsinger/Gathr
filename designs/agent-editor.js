
const option = document.body.dataset.agentOption;
const fixtureAgents = [
  { name: 'Mira', engine: 'Claude Code', account: 'Main', model: 'Engine default', effort: 'Engine default' },
  { name: 'Rowan', engine: 'Codex', account: 'Studio', model: 'Engine default', effort: 'Engine default' },
  { name: 'Kit', engine: 'Claude Code', account: 'Studio', model: 'Engine default', effort: 'Engine default' },
  { name: 'Sol', engine: 'Codex', account: 'Main', model: 'Engine default', effort: 'Engine default' },
  { name: 'Lane', engine: 'Claude Code', account: 'Main', model: 'Engine default', effort: 'Engine default' }
];
let selectedAgent = 0;
let editorOpener;
const settings = document.createElement('dialog');
settings.className = 'settings-panel';
settings.setAttribute('aria-label', 'Settings');
settings.dataset.id = 'agent-settings';
settings.innerHTML = `<header class="settings-header"><h2>Settings</h2><button id="agent-close">Close</button></header>
<div class="settings-body"><nav class="settings-categories" aria-label="Settings categories">
<button aria-current="page">Agents</button></nav><div class="agent-workspace" id="settings-editor"></div></div>`;
document.body.append(settings);
const menu = document.createElement('dialog');
menu.className = 'agent-menu';
menu.setAttribute('aria-label', 'Edit agent');
document.body.append(menu);
function openAgentSettings() {
  editorOpener = document.activeElement === document.body ? settingsLink : document.activeElement;
  renderEditor(settings.querySelector('#settings-editor'));
  settings.showModal();
}
settings.querySelector('#agent-close').onclick = () => settings.close();
for (const dialog of [settings, menu]) dialog.addEventListener('close', () => editorOpener?.focus());
const settingsLink = document.createElement('button');
settingsLink.textContent = 'Agents';
settingsLink.dataset.id = 'open-agent-settings';
settingsLink.onclick = openAgentSettings;
document.querySelector('.brand').after(settingsLink);
function renderEditor(host, inline = false) {
  const agent = fixtureAgents[selectedAgent];
  host.innerHTML = `<nav class="agent-list" aria-label="Agents"></nav><form class="agent-form">
<h3>${agent.name}</h3>
<label class="agent-field">Engine<select name="engine"><option>Claude Code</option><option>Codex</option></select></label>
<label class="agent-field">Account<select name="account"><option>Main</option><option>Studio</option></select></label>
<label class="agent-field">Model<output id="chosen-model"></output>
<input type="search" name="modelSearch" aria-label="Find models" placeholder="Find models"></label>
<div class="model-results" aria-label="Models"></div>
<label class="agent-field">Effort<select name="effort"><option>Engine default</option><option>Sample low</option>
<option>Sample medium</option><option>Sample high</option><option>Sample maximum</option></select></label>
<details><summary>Scope</summary><p>All chats, from the next turn.</p></details>
<p class="agent-preview-note">Sample agents, accounts and engine choices. Local preview.</p>
<div class="agent-actions"><button type="button" id="agent-cancel">Cancel</button><button type="submit">Save</button></div>
</form>`;
  fixtureAgents.forEach((item, index) => {
    const button = document.createElement('button');
    button.textContent = item.name;
    button.dataset.id = 'edit-agent-' + index;
    if (index === selectedAgent) button.setAttribute('aria-current', 'true');
    button.onclick = () => { selectedAgent = index; renderEditor(host, inline); };
    host.querySelector('.agent-list').append(button);
  });
  const form = host.querySelector('form');
  for (const field of ['engine', 'account', 'effort']) form.elements[field].value = agent[field];
  let chosenModel = agent.model;
  function models() {
    host.querySelector('#chosen-model').textContent = chosenModel;
    const results = host.querySelector('.model-results');
    results.replaceChildren();
    const prefix = form.elements.engine.value === 'Codex' ? 'Codex' : 'Claude';
    const family = prefix === 'Claude' ? ['Sample Opus Latest', 'Sample Opus 4.6'] : [];
    const names = ['Engine default', ...family, ...Array.from({ length: 24 }, (_, i) => `Sample ${prefix} model ${String(i + 1).padStart(2, '0')}`)];
    const filtered = names.filter(name => name.toLowerCase().includes(form.elements.modelSearch.value.toLowerCase()));
    for (const name of filtered) {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = name;
      button.setAttribute('aria-pressed', String(name === chosenModel));
      button.onclick = () => { chosenModel = name; models(); };
      results.append(button);
    }
    if (!filtered.length) results.textContent = 'No models found';
  }
  form.elements.engine.onchange = () => {
    form.elements.account.value = 'Main';
    form.elements.effort.value = 'Engine default';
    form.elements.modelSearch.value = '';
    chosenModel = 'Engine default';
    models();
  };
  form.elements.modelSearch.oninput = models;
  function closeEditor() {
    if (settings.open) settings.close();
    if (menu.open) menu.close();
    if (inline) { host.hidden = true; editorOpener?.focus(); }
  }
  form.querySelector('#agent-cancel').onclick = closeEditor;
  form.onsubmit = event => {
    event.preventDefault();
    Object.assign(agent, { engine: form.elements.engine.value, account: form.elements.account.value,
      model: chosenModel, effort: form.elements.effort.value });
    closeEditor();
    notify(agent.name + ' saved');
  };
  models();
}
if (option === 'settings') openAgentSettings();
if (option === 'team') {
  const panel = document.querySelector('.panel');
  panel.hidden = false;
  const editor = document.createElement('section');
  editor.className = 'agent-inline';
  editor.innerHTML = '<h2 style="padding:16px">Edit agent</h2><div class="agent-workspace"></div>';
  panel.prepend(editor);
  const button = document.createElement('button');
  button.textContent = 'Edit agents';
  button.dataset.id = 'open-team-editor';
  button.onclick = () => {
    editorOpener = button;
    editor.querySelector('.agent-workspace').hidden = false;
    renderEditor(editor.querySelector('.agent-workspace'), true);
  };
  editor.prepend(button);
  editorOpener = button;
  renderEditor(editor.querySelector('.agent-workspace'), true);
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && !settings.open && !menu.open && !editor.querySelector('.agent-workspace').hidden) {
      editor.querySelector('.agent-workspace').hidden = true;
      button.focus();
    }
  });
}
if (option === 'menu') {
  const button = document.createElement('button');
  button.textContent = 'Edit agent';
  button.dataset.id = 'open-chat-agent';
  button.onclick = () => {
    editorOpener = button;
    const picker = document.querySelector('[aria-label="Agent"]');
    selectedAgent = Math.max(0, fixtureAgents.findIndex(agent => agent.name === picker.value));
    renderEditor(menu);
    menu.showModal();
  };
  document.querySelector('[aria-label="Agent"]').after(button);
  button.click();
}
