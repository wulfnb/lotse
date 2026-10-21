const form = document.querySelector('#login-form');
form.addEventListener('submit', async event => {
  event.preventDefault();
  const button = form.querySelector('button');
  const notice = document.querySelector('#login-notice');
  button.disabled = true;
  notice.textContent = 'Signing in…';
  try {
    const response = await fetch('/auth/login', {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({username: form.elements.username.value, password: form.elements.password.value})
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Could not sign in.');
    const session = await fetch('/auth/account', {credentials: 'same-origin', cache: 'no-store'});
    if (!session.ok) {
      throw new Error('Your password was accepted, but the session cookie was not saved. Allow cookies for this HTTPS address and try again.');
    }
    // Bypass login documents cached by an earlier version of the viewer.
    location.replace(`/?signed-in=${Date.now()}`);
  } catch (error) { notice.textContent = error.message; }
  finally { form.elements.password.value = ''; button.disabled = false; }
});
