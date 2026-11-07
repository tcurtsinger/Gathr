const initialScreen = screen;
engines();
const enginesPage = document.createElement('section');
enginesPage.className = 'settings-page';
enginesPage.append(...surface.childNodes);
diagnostics();
const diagnosticsPage = document.createElement('section');
diagnosticsPage.className = 'settings-page diagnostics-page';
diagnosticsPage.append(...surface.childNodes);
chat();
const settingsPanel = document.createElement('dialog');
settingsPanel.className = 'settings-panel';
settingsPanel.dataset.id = 'settings-panel';
settingsPanel.setAttribute('aria-labelledby', 'settings-title');
settingsPanel.innerHTML = `<header class="settings-header" data-id="settings-header">
  <h2 id="settings-title" data-id="settings-title">Settings</h2>
  <button id="close-settings" data-id="close-settings" aria-label="Close settings" autofocus>Close</button></header>
  <div class="settings-body"><nav class="settings-categories" aria-label="Settings" data-id="settings-categories">
  <button data-id="settings-engines" data-category="engines">Engines</button>
  <button data-id="settings-diagnostics" data-category="diagnostics">Diagnostics</button></nav>
  <div class="settings-scroll" data-id="settings-scroll"></div></div>`;
settingsPanel.querySelector('.settings-scroll').append(enginesPage, diagnosticsPage);
surface.append(settingsPanel);
let settingsReturn;
function openSettings(category, reference, opener) {
  settingsReturn = opener || document.activeElement;
  enginesPage.hidden = category !== 'engines';
  diagnosticsPage.hidden = category !== 'diagnostics';
  settingsPanel.querySelectorAll('[data-category]').forEach(button => {
    if (button.dataset.category === category) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  });
  if (category === 'diagnostics' && reference) {
    const find = diagnosticsPage.querySelector('#find');
    find.value = reference;
    find.dispatchEvent(new Event('input'));
  }
  if (!settingsPanel.open) settingsPanel.showModal();
}
settingsPanel.querySelectorAll('[data-category]').forEach(button => {
  button.onclick = () => openSettings(button.dataset.category, null, button);
});
settingsPanel.querySelector('#close-settings').onclick = () => settingsPanel.close();
settingsPanel.addEventListener('close', () => {
  const target = settingsReturn?.isConnected && !settingsPanel.contains(settingsReturn)
    ? settingsReturn : document.querySelector('.profile button');
  (target.getClientRects().length ? target : document.querySelector('.sidebar-reveal')).focus();
});
document.querySelector('.profile button').onclick = e => openSettings('engines', null, e.currentTarget);
document.querySelectorAll('.topbar a').forEach(link => {
  if (link.textContent === 'Diagnostics' || link.textContent === 'Engines') {
    link.onclick = e => { e.preventDefault(); openSettings(link.textContent.toLowerCase(), null, link); };
  }
});
// Generated reply notices are replaced when the preview changes state.
surface.addEventListener('click', e => {
  const link = e.target.closest('.chat-stage .notice a, .chat-stage .notice .ref');
  if (!link) return;
  e.preventDefault();
  openSettings(link.textContent === 'Sign in' ? 'engines' : 'diagnostics',
    link.textContent === 'Sign in' ? null : sendRef, link);
});
document.querySelector('#signin').addEventListener('close', () => document.querySelector('#sign-in').focus());
if (initialScreen !== 'chat-states') openSettings(initialScreen);
if (initialScreen === 'chat-states') {
  const preview = document.querySelector('#chat-preview');
  preview.value = 'Error'; preview.dispatchEvent(new Event('change'));
}
