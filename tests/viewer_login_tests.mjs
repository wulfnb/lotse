import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';
const filename = fileURLToPath(new URL('../docker/viewer/login.js', import.meta.url));
const source = readFileSync(filename, 'utf8');

function fixture(responses) {
  const button = {disabled: false};
  const notice = {textContent: ''};
  const calls = [];
  const locations = [];
  let submit;
  const form = {
    elements: {username: {value: 'alice'}, password: {value: 'test-password'}},
    querySelector: selector => { assert.equal(selector, 'button'); return button; },
    addEventListener: (event, callback) => { assert.equal(event, 'submit'); submit = callback; }
  };
  const context = {
    document: {querySelector: selector => selector === '#login-form' ? form : notice},
    fetch: async (...args) => {
      calls.push(args);
      const response = responses.shift();
      if (response instanceof Error) throw response;
      return response;
    },
    location: {replace: value => locations.push(value)},
    Date: {now: () => 1234}
  };
  new vm.Script(source, {filename}).runInNewContext(context);
  return {button, notice, form, calls, locations, submit};
}

test('login verifies the session cookie before redirect and clears the password', async () => {
  const f = fixture([{ok: true, json: async () => ({})}, {ok: true}]);
  let prevented = false;
  await f.submit({preventDefault: () => { prevented = true; }});
  assert.equal(prevented, true);
  assert.deepEqual(f.locations, ['/?signed-in=1234']);
  assert.equal(f.calls[0][0], '/auth/login');
  assert.deepEqual(JSON.parse(f.calls[0][1].body), {username: 'alice', password: 'test-password'});
  assert.equal(f.calls[0][1].credentials, 'same-origin');
  assert.equal(f.calls[1][0], '/auth/account');
  assert.equal(f.form.elements.password.value, '');
  assert.equal(f.button.disabled, false);
});

for (const [name, responses, message] of [
  ['invalid credentials', [{ok: false, json: async () => ({error: 'Incorrect credentials'})}], 'Incorrect credentials'],
  ['unexplained server rejection', [{ok: false, json: async () => ({})}], 'Could not sign in.'],
  ['missing session cookie', [{ok: true, json: async () => ({})}, {ok: false}], 'Your password was accepted'],
  ['network failure', [new Error('Network unavailable')], 'Network unavailable'],
  ['invalid JSON', [{ok: false, json: async () => {throw new Error('Invalid response');}}], 'Invalid response']
]) {
  test(name + ' never redirects and restores controls', async () => {
    const f = fixture(responses);
    await f.submit({preventDefault() {}});
    assert.ok(f.notice.textContent.startsWith(message));
    assert.deepEqual(f.locations, []);
    assert.equal(f.form.elements.password.value, '');
    assert.equal(f.button.disabled, false);
  });
}
