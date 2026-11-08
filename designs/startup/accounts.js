const $ = selector => document.querySelector(selector);
const accountFixtures = [
  {id: 1, name: 'Personal', engine: 'Claude Code', signedIn: true},
  {id: 2, name: 'Work', engine: 'Claude Code', signedIn: false}
];
let nextAccount = 3;
let editingAccount;
let signingAccount;
let opener;
const dialog = $('#settings');
function openSettings(page = 'engines') {
  if (!dialog.open) opener = document.activeElement;
  for (const id of ['general', 'engines', 'agents', 'diagnostics']) { const section = document.getElementById(id); if (section) section.hidden = id !== page; }
  document.querySelectorAll('[data-page]').forEach(button => {
    if (button.dataset.page === page) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  });
  if (page === 'agents') renderEditor($('#settings-editor'));
  if (!dialog.open) dialog.showModal();
}
document.querySelectorAll('[data-open]').forEach(b => b.onclick = () => openSettings(b.dataset.open));
document.querySelectorAll('[data-page]').forEach(b => b.onclick = () => openSettings(b.dataset.page));
$('#close').onclick = () => dialog.close();
dialog.addEventListener('close', () => { $('#account-form').hidden = true; opener?.focus(); });
function renderAccounts() {
  $('#account-list').replaceChildren();
  if (!accountFixtures.length) $('#account-list').textContent = 'No accounts';
  for (const account of accountFixtures) {
    const row = document.createElement('div');
    row.className = 'account-row';
    row.dataset.id = 'account-row-' + account.id;
    const label = document.createElement('div');
    const name = document.createElement('strong');
    name.textContent = account.name;
    const state = document.createElement('small');
    state.textContent = account.engine + ' / ' + (signingAccount === account.id ? 'Signing in' : account.signedIn ? 'Signed in' : 'Signed out');
    label.append(name, state);
    const actions = document.createElement('div');
    actions.className = 'account-actions';
    const rename = document.createElement('button');
    rename.textContent = 'Rename';
    rename.onclick = () => showAccountForm(account);
    const sign = document.createElement('button');
    sign.textContent = signingAccount === account.id ? 'Cancel' : account.signedIn ? 'Sign out' : 'Sign in';
    sign.onclick = () => {
      if (signingAccount === account.id) { signingAccount = undefined; renderAccounts(); }
      else if (account.signedIn) { account.signedIn = false; renderAccounts(); }
      else beginSignIn(account);
    };
    actions.append(rename, sign);
    if (signingAccount === account.id) {
      const finish = document.createElement('button');
      finish.textContent = 'Simulate success';
      finish.onclick = () => {
        account.signedIn = true;
        signingAccount = undefined;
        if (account.engine === 'Codex') $('[data-id="codex-version"] .rowend').textContent = 'Installed (sample)';
        renderAccounts();
      };
      actions.append(finish);
    }
    row.append(label, actions);
    $('#account-list').append(row);
  }
}
function beginSignIn(account) {
  signingAccount = account.id;
  $('#account-message').textContent = 'Sign in in your browser.';
  renderAccounts();
}
function showAccountForm(account) {
  editingAccount = account;
  $('#account-form-title').textContent = account ? 'Rename account' : 'Add account';
  $('#account-engine').disabled = !!account;
  $('#account-engine').value = account?.engine || 'Claude Code';
  $('#account-name').value = account?.name || '';
  $('#account-name').setCustomValidity('');
  $('#save-account').textContent = account ? 'Save' : 'Sign in';
  $('#account-form').hidden = false;
  $('#account-name').focus();
}
$('#add-account').onclick = () => showAccountForm();
$('#cancel-account').onclick = () => { $('#account-form').hidden = true; $('#add-account').focus(); };
$('#account-form').onsubmit = event => {
  event.preventDefault();
  const name = $('#account-name').value.trim();
  if (!name) { $('#account-name').setCustomValidity('Enter a name'); $('#account-name').reportValidity(); return; }
  if (editingAccount) { editingAccount.name = name; renderAccounts(); }
  else {
    const account = {id: nextAccount++, name, engine: $('#account-engine').value, signedIn: false};
    accountFixtures.push(account);
    beginSignIn(account);
  }
  $('#account-form').hidden = true;
  $('#add-account').focus();
};
$('#account-name').oninput = () => $('#account-name').setCustomValidity('');
$('#theme').onclick = () => document.documentElement.classList.toggle('light');
if (location.hash === '#light') document.documentElement.classList.add('light');
$('#reply-state').onchange = event => {
  $('#notice').hidden = event.target.value === 'complete';
  $('#notice-label').textContent = event.target.value === 'failed' ? 'Send failed' : 'Stopped';
};
$('#engine-state').onchange = event => {
  const state = event.target.value;
  $('#engine-status').textContent = {current:'Up to date', available:'Update available', updating:'Updating', failed:'Update failed'}[state];
  $('#update').textContent = state === 'available' ? 'Update' : 'Check';
  $('#update').disabled = state === 'updating';
  $('#engine-failure').hidden = state !== 'failed';
};
$('#update').onclick = () => {
  $('#engine-status').textContent = 'Up to date';
  $('#update').textContent = 'Check';
  $('#engine-failure').hidden = true;
};
$('#new').onclick = () => { $('.user').textContent = 'New chat'; $('.reply').hidden = true; $('#draft').value = ''; $('#draft').focus(); };
$('#send').onclick = () => {
  if ($('#draft').value.trim()) { $('.user').textContent = $('#draft').value; $('.reply').hidden = false; $('#draft').value = ''; }
};
function notify(message) { $('#account-message').textContent = message; }
renderAccounts();
