import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';
const filename = fileURLToPath(new URL('../docker/viewer/viewer.js', import.meta.url));
class Element {
  constructor() { this.listeners = new Map(); this.value = ''; this.children = []; this.hidden = false; this.passwords = [{value: 'secret'}]; this.elements = {username: {value: ''}}; this.files = []; this.classList = {toggle: (name, value) => {this[name] = value;}}; }
  addEventListener(event, callback) { this.listeners.set(event, callback); }
  async fire(event, detail = {}) { await this.listeners.get(event)?.({preventDefault() {}, detail}); await new Promise(setImmediate); }
  replaceChildren() { this.children = []; }
  append(...children) { this.children.push(...children); }
  showModal() { this.open = true; }
  close() { this.open = false; this.listeners.get('close')?.(); }
  querySelector() { return this.button ||= {disabled: false}; }
  querySelectorAll() { return this.passwords; }
  focus() {this.focused = true;}
  select() {this.selected = true;}
  reset() { this.resets = (this.resets || 0) + 1; }
}
async function fixture() {
  const elements = new Map();
  const get = id => { if (!elements.has(id)) elements.set(id, new Element()); return elements.get(id); };
  const clients = [], calls = [], replies = [], locations = [], timers = [];
  class RFB extends Element {
    constructor(screen, url, options) { super(); clients.push(this); this.url = url; this.options = options; }
    focus() { this.focused = true; }
    blur() { this.focused = false; }
    disconnect() { this.disconnected = true; }
    clipboardPasteFrom(value) { this.pasted = value; }
  }
  const document = {querySelector: get, createElement: () => new Element(), documentElement: {requestFullscreen: async () => {document.fullscreenElement = true;}}, exitFullscreen: async () => {document.fullscreenElement = false;}};
  const context = vm.createContext({document, location: {host: 'test.local', replace: url => locations.push(url)}, fetch: async (...args) => {
    calls.push(args);
    const response = replies.shift();
    if (response instanceof Error) throw response;
    if (typeof response === 'function') return await response();
    assert.ok(response, 'Unexpected fetch ' + args[0]);
    return response;
  }, FormData: class { constructor(form) { return new Map([['username', form.elements.username.value], ['password', 'new-secret']]); } }, setTimeout: callback => timers.push(callback)});
  const module = new vm.SourceTextModule(readFileSync(filename, 'utf8'), {context, identifier: filename});
  await module.link(() => new vm.SyntheticModule(['default'], function () { this.setExport('default', RFB); }, {context}));
  await module.evaluate();
  return {get, document, clients, calls, replies, locations, timers};
}
const ok = value => ({ok: true, status: 200, json: async () => value});

test('connection events ignore stale clients and reveal authentication expiry', async () => {
  const f = await fixture(); const first = f.clients[0];
  assert.equal(first.url, 'wss://test.local/websockify');
  await first.fire('connect'); assert.equal(f.get('#status').textContent, 'Connected');
  await first.fire('securityfailure'); assert.equal(f.get('#status').textContent, 'Connection refused');
  await first.fire('clipboard', {text: 'from browser'}); assert.equal(f.get('#clipboard-text').value, 'from browser');
  await f.get('#reconnect').fire('click'); assert.equal(first.disconnected, true);
  await first.fire('connect'); await first.fire('disconnect'); await first.fire('securityfailure');
  assert.equal(f.get('#status').textContent, 'Connecting…');
  const second = f.clients[1];
  f.replies.push({status: 401}); await second.fire('disconnect'); assert.deepEqual(f.locations, ['/login.html']);
  f.replies.push(ok({})); await second.fire('disconnect');
  f.replies.push(new Error('offline')); await second.fire('disconnect');
  assert.equal(f.get('#reconnect').hidden, false);
});

test('fullscreen failure and clipboard controls restore focus', async () => {
  const f = await fixture();
  await f.get('#fullscreen').fire('click'); assert.equal(f.document.fullscreenElement, true);
  await f.get('#fullscreen').fire('click'); assert.equal(f.document.fullscreenElement, false);
  f.document.documentElement.requestFullscreen = async () => {throw new Error('denied');};
  await f.get('#fullscreen').fire('click'); assert.equal(f.get('#status').textContent, 'Full screen unavailable');
  await f.get('#clipboard').fire('click'); assert.equal(f.get('#clipboard-dialog').open, true);
  f.get('#clipboard-text').value = 'to browser';
  await f.get('#send-clipboard').fire('click'); assert.equal(f.clients[0].pasted, 'to browser'); assert.equal(f.clients[0].focused, true);
});

test('downloads show only completed files with safe URLs and handle errors', async () => {
  const f = await fixture();
  f.replies.push(ok([{type: 'file', name: 'z file.bin', size: 1048576}, {type: 'directory', name: 'directory'}, {type: 'file', name: '.partial'}, {type: 'file', name: 'a.bin', size: 0}]));
  await f.get('#downloads').fire('click');
  const rows = f.get('#downloads-list').children;
  assert.equal(rows.length, 2); assert.equal(rows[0].children[0].textContent, 'a.bin');
  assert.equal(rows[1].children[0].href, '/downloads/z%20file.bin');
  assert.equal(rows[1].children[1].textContent, '1.0 MiB');
  f.replies.push(ok([])); await f.get('#refresh-downloads').fire('click'); assert.ok(f.get('#downloads-notice').textContent.startsWith('No completed'));
  f.replies.push({ok: false, status: 503}); await f.get('#refresh-downloads').fire('click'); assert.ok(f.get('#downloads-notice').textContent.includes('HTTP 503'));
  await f.get('#close-downloads').fire('click'); assert.equal(f.clients[0].focused, true);
});

test('account loading errors, saving and signout clear secrets and check CSRF', async () => {
  const f = await fixture();
  f.replies.push({ok: false, status: 401}); await f.get('#account').fire('click'); assert.ok(f.get('#account-notice').textContent.includes('Please sign in'));
  f.replies.push({ok: false, status: 503}); await f.get('#account').fire('click'); assert.ok(f.get('#account-notice').textContent.includes('Could not load'));
  f.replies.push(ok({username: 'alice', csrf: 'csrf-token'})); await f.get('#account').fire('click');
  assert.equal(f.get('#account-form').elements.username.value, 'alice');
  f.replies.push(ok({})); await f.get('#account-form').fire('submit');
  const post = f.calls.at(-1); assert.equal(post[1].headers['X-CSRF-Token'], 'csrf-token');
  assert.equal(f.get('#account-form').passwords[0].value, ''); assert.equal(f.get('#account-form').button.disabled, false);
  assert.equal(f.get('#status').textContent, 'Account updated');
  f.timers[0](); assert.equal(f.clients.length, 2);
  for (const result of [{error: 'Rejected'}, {}]) {
    f.replies.push({ok: false, json: async () => result}); await f.get('#account-form').fire('submit');
    assert.ok(f.get('#account-notice').textContent.includes(result.error || 'Could not save'));
  }
  await f.get('#close-account').fire('click');
  f.replies.push(ok({csrf: 'logout-token'}), {ok: false}); await f.get('#signout').fire('click'); assert.equal(f.get('#status').textContent, 'Could not sign out.');
  f.replies.push(ok({csrf: 'logout-token'}), ok({})); await f.get('#signout').fire('click');
  assert.equal(f.calls.at(-1)[1].headers['X-CSRF-Token'], 'logout-token');
  assert.equal(f.locations.at(-1), '/login.html'); assert.equal(f.clients.at(-1).disconnected, true);
});

test('bridge disconnect during credential rotation does not redirect or check old cookie', async () => {
  const f = await fixture();
  f.replies.push(async () => { await f.clients[0].fire('disconnect'); return ok({}); });
  await f.get('#account-form').fire('submit');
  assert.equal(f.calls.length, 1); assert.deepEqual(f.locations, []);
});


test('viewport modes allow actual-size panning and explicit remote resizing', async () => {
  const f = await fixture();
  const mode = f.get('#viewport-mode');
  for (const value of ['actual', 'resize', 'fit']) {
    mode.value = value; await mode.fire('change');
    assert.equal(f.clients[0].scaleViewport, value !== 'actual');
    assert.equal(f.clients[0].resizeSession, value === 'resize');
    assert.equal(f.get('#screen')['actual-size'], value === 'actual');
  }
});

test('uploads are bounded, authenticated and return a selectable remote path', async () => {
  const f = await fixture();
  f.replies.push(ok({csrf:'upload-token'})); await f.get('#upload').fire('click');
  assert.equal(f.get('#upload-dialog').open, true);
  await f.get('#upload-form').fire('submit');
  assert.match(f.get('#upload-notice').textContent, /Choose a file/);
  f.get('#upload-file').files = [{name:'large', size:33*1024*1024}];
  await f.get('#upload-form').fire('submit'); assert.equal(f.calls.length, 1);
  f.get('#upload-file').files = [{name:'a b.txt', size:4}];
  f.replies.push(ok({path:'/config/auth/uploads/file-123/a b.txt'}));
  await f.get('#upload-form').fire('submit');
  assert.equal(f.calls[1][0], '/auth/upload');
  assert.equal(f.calls[1][1].headers['X-CSRF-Token'], 'upload-token');
  assert.equal(f.calls[1][1].headers['X-Upload-Name'], 'a%20b.txt');
  assert.equal(f.get('#upload-path').selected, true);
  assert.match(f.get('#upload-notice').textContent, /Uploaded/);
  f.replies.push({ok:false,json:async()=>({error:'Storage full'})});
  await f.get('#upload-form').fire('submit'); assert.equal(f.get('#upload-notice').textContent, 'Storage full');
  f.replies.push({ok:false,json:async()=>({})});
  await f.get('#upload-form').fire('submit'); assert.equal(f.get('#upload-notice').textContent, 'Upload failed.');
  f.replies.push(new Error('offline')); await f.get('#upload-form').fire('submit');
  assert.equal(f.get('#upload-notice').textContent, 'offline');
  await f.get('#close-upload').fire('click'); assert.equal(f.clients[0].focused, true);
  f.replies.push(new Error('offline')); await f.get('#upload').fire('click');
  assert.equal(f.get('#upload-notice').textContent, 'offline');
});
