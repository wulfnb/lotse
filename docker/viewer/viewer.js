import RFB from '/novnc/core/rfb.js';
const status = document.querySelector('#status');
const reconnect = document.querySelector('#reconnect');
const screen = document.querySelector('#screen');
const dialog = document.querySelector('#clipboard-dialog');
const text = document.querySelector('#clipboard-text');
let rfb;
let updatingAccount = false;
function connect() {
  reconnect.hidden = true;
  status.textContent = 'Connecting…';
  if (rfb) rfb.disconnect();
  screen.replaceChildren();
  const client = new RFB(screen, `wss://${location.host}/websockify`, {shared: true});
  rfb = client;
  const mode = document.querySelector('#viewport-mode').value;
  client.scaleViewport = mode !== 'actual';
  client.resizeSession = mode === 'resize';
  client.qualityLevel = 6;
  client.compressionLevel = 2;
  client.addEventListener('connect', () => {
    if (rfb !== client) return;
    status.textContent = 'Connected';
    client.focus();
  });
  client.addEventListener('disconnect', () => {
    if (rfb !== client) return;
    status.textContent = 'Disconnected';
    reconnect.hidden = false;
    if (updatingAccount) return;
    fetch('/auth/account', {cache: 'no-store'}).then(response => {
      if (response.status === 401) location.replace('/login.html');
    }).catch(() => {});
  });
  client.addEventListener('securityfailure', () => {
    if (rfb !== client) return;
    status.textContent = 'Connection refused';
    reconnect.hidden = false;
  });
  client.addEventListener('clipboard', event => {text.value = event.detail.text;});
}
reconnect.addEventListener('click', connect);
document.querySelector('#fullscreen').addEventListener('click', async () => {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  } catch { status.textContent = 'Full screen unavailable'; }
});
document.querySelector('#clipboard').addEventListener('click', () => {rfb?.blur();dialog.showModal();});
dialog.addEventListener('close', () => rfb?.focus());
document.querySelector('#send-clipboard').addEventListener('click', () => {
  rfb?.clipboardPasteFrom(text.value);
  dialog.close();
});
connect();
const downloadsDialog = document.querySelector('#downloads-dialog');
async function listDownloads() {
  const notice = document.querySelector('#downloads-notice');
  const list = document.querySelector('#downloads-list');
  notice.textContent = 'Loading…';
  list.replaceChildren();
  try {
    const response = await fetch('/downloads/', {cache: 'no-store'});
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const files = (await response.json()).filter(f => f.type === 'file' && !f.name.startsWith('.'));
    files.sort((a, b) => a.name.localeCompare(b.name));
    for (const file of files) {
      const row = document.createElement('li');
      const link = document.createElement('a');
      link.textContent = file.name;
      link.href = `/downloads/${encodeURIComponent(file.name)}`;
      link.download = file.name;
      const size = document.createElement('span');
      size.textContent = `${(file.size / 1048576).toFixed(1)} MiB`;
      row.append(link, size);
      list.append(row);
    }
    notice.textContent = files.length ? `${files.length} completed file(s)` : 'No completed downloads yet. Use ↓ in the native browser to start a download.';
  } catch (error) { notice.textContent = `Could not list downloads: ${error.message}`; }
}
document.querySelector('#downloads').addEventListener('click', () => { rfb?.blur(); downloadsDialog.showModal(); listDownloads(); });
document.querySelector('#refresh-downloads').addEventListener('click', listDownloads);
document.querySelector('#close-downloads').addEventListener('click', () => downloadsDialog.close());
downloadsDialog.addEventListener('close', () => rfb?.focus());

const accountDialog = document.querySelector('#account-dialog');
const accountForm = document.querySelector('#account-form');
const accountNotice = document.querySelector('#account-notice');
let csrf = '';
async function loadAccount() {
  const response = await fetch('/auth/account', {cache: 'no-store'});
  if (response.status === 401) { location.replace('/login.html'); throw new Error('Please sign in.'); }
  if (!response.ok) throw new Error('Could not load account settings.');
  const result = await response.json();
  csrf = result.csrf;
  return result;
}
document.querySelector('#account').addEventListener('click', async () => {
  rfb?.blur();
  accountForm.reset();
  accountNotice.textContent = 'Loading…';
  accountDialog.showModal();
  try { accountForm.elements.username.value = (await loadAccount()).username; accountNotice.textContent = ''; }
  catch (error) { accountNotice.textContent = error.message; }
});
accountForm.addEventListener('submit', async event => {
  event.preventDefault();
  const button = accountForm.querySelector('button[type="submit"]');
  button.disabled = true;
  updatingAccount = true;
  accountNotice.textContent = 'Saving…';
  try {
    const response = await fetch('/auth/account', {
      method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': csrf},
      body: JSON.stringify(Object.fromEntries(new FormData(accountForm)))
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Could not save account settings.');
    status.textContent = 'Account updated';
    accountDialog.close();
    setTimeout(connect, 700);
  } catch (error) { accountNotice.textContent = error.message; }
  finally {
    for (const input of accountForm.querySelectorAll('input[type="password"]')) input.value = '';
    button.disabled = false;
    updatingAccount = false;
  }
});
document.querySelector('#close-account').addEventListener('click', () => accountDialog.close());
accountDialog.addEventListener('close', () => { accountForm.reset(); csrf = ''; rfb?.focus(); });
document.querySelector('#signout').addEventListener('click', async () => {
  try {
    await loadAccount();
    const response = await fetch('/auth/logout', {
      method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': csrf}, body: '{}'
    });
    if (!response.ok) throw new Error('Could not sign out.');
    rfb?.disconnect();
    location.replace('/login.html');
  } catch (error) { status.textContent = error.message; }
});

const viewportMode = document.querySelector('#viewport-mode');
viewportMode.addEventListener('change', () => {
  screen.classList.toggle('actual-size', viewportMode.value === 'actual');
  rfb.scaleViewport = viewportMode.value !== 'actual';
  rfb.resizeSession = viewportMode.value === 'resize';
  status.textContent = viewportMode.value === 'actual' ? 'Actual size: scroll to pan the desktop' : viewportMode.value === 'resize' ? 'Remote resize requested; use Fit if unsupported' : 'Fit to window';
});
const uploadDialog = document.querySelector('#upload-dialog');
const uploadForm = document.querySelector('#upload-form');
const uploadNotice = document.querySelector('#upload-notice');
const uploadPath = document.querySelector('#upload-path');
let uploadCsrf = '';
document.querySelector('#upload').addEventListener('click', async () => {
  rfb?.blur(); uploadForm.reset(); uploadPath.value = ''; uploadNotice.textContent = 'Loading…'; uploadDialog.showModal();
  try { uploadCsrf = (await loadAccount()).csrf; uploadNotice.textContent = ''; }
  catch (error) { uploadNotice.textContent = error.message; }
});
uploadForm.addEventListener('submit', async event => {
  event.preventDefault();
  const file = document.querySelector('#upload-file').files[0];
  if (!file || file.size > 32 * 1024 * 1024) { uploadNotice.textContent = 'Choose a file of 32 MiB or smaller.'; return; }
  const button = uploadForm.querySelector('button[type="submit"]'); button.disabled = true;
  uploadNotice.textContent = 'Uploading…'; uploadPath.value = '';
  try {
    const response = await fetch('/auth/upload', {method:'POST', headers:{'Content-Type':'application/octet-stream', 'X-CSRF-Token':uploadCsrf, 'X-Upload-Name':encodeURIComponent(file.name)}, body:file});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Upload failed.');
    uploadPath.value = result.path; uploadPath.focus(); uploadPath.select();
    uploadNotice.textContent = 'Uploaded. Copy this remote path into the website file chooser.';
  } catch (error) { uploadNotice.textContent = error.message; }
  finally { button.disabled = false; }
});
document.querySelector('#close-upload').addEventListener('click', () => uploadDialog.close());
uploadDialog.addEventListener('close', () => { uploadForm.reset(); uploadCsrf = ''; rfb?.focus(); });
