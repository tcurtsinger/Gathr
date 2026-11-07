// This version changes where Settings opens, keeping the existing engine form.
const engineForm = surface.querySelector('.settings');
const panel = document.createElement('dialog');
panel.id = 'settings-panel';
panel.className = 'settings-panel';
panel.setAttribute('aria-labelledby', 'settings-title');
panel.dataset.id = 'settings-panel';
panel.innerHTML = `<header class="settings-header" data-id="settings-header">
  <h2 id="settings-title" data-id="settings-title">Settings</h2>
  <button id="close-settings" data-id="close-settings" aria-label="Close settings" autofocus>
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
    <path d="m6 6 12 12M18 6 6 18"/></svg></button></header>
  <div class="settings-body"><nav class="settings-categories" aria-label="Settings" data-id="settings-categories">
  <button aria-current="page" data-id="settings-engines">Engines</button></nav>
  <div class="settings-scroll" data-id="settings-scroll"></div></div>`;
panel.querySelector('.settings-scroll').append(engineForm);
// The underlying chat stays in place when the dialog opens and closes.
surface.innerHTML = `<div class="chat-stage"><div class="transcript"><div class="reading">
  <article class="message user"><div class="content"><p>Check the reconnect path and keep the draft intact.</p></div></article>
  <article class="message"><div class="who">Assistant</div><p>The draft is saved before reconnect starts.
  I found a second path that clears it when the socket closes.</p>
  <details><summary>12 tool calls</summary><p>Read reconnect tests, search handlers, inspect draft storage.</p></details></article>
  </div></div><div class="composerarea"><div class="reading"><div class="composer">
  <textarea aria-label="Message" placeholder="Message"></textarea><div class="composerbottom">
  <button class="primary" id="background-send">Send</button></div></div></div></div></div>`;
surface.append(panel);
const settingsOpener = document.querySelector('.profile button');
settingsOpener.dataset.id = 'open-settings';
settingsOpener.onclick = () => panel.showModal();
document.querySelector('#close-settings').onclick = () => panel.close();
panel.addEventListener('close', () => {
  const target = settingsOpener.getClientRects().length ? settingsOpener : document.querySelector('.sidebar-reveal');
  target.focus();
});
document.querySelector('[data-id="settings-engines"]').onclick = () => panel.querySelector('h2 + .preview-state select')?.focus();
document.querySelector('#background-send').onclick = () => notify('Sent, preview only');
// The nested sign-in preview uses the native dialog stack and returns to Settings.
document.querySelector('#signin').addEventListener('close', () => document.querySelector('#sign-in').focus());
panel.showModal();
