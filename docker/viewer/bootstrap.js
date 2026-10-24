// A classic script can report a failed module download or evaluation.
(() => {
  let settled = false;
  const showFailure = () => {
    if (settled) return;
    settled = true;
    document.querySelector('#connection-card').hidden = false;
    document.querySelector('#connection-message').textContent = 'Browser controls could not load. Reload the page to try again.';
    document.querySelector('#status').textContent = 'Browser controls unavailable';
    const retry = document.querySelector('#retry-connection');
    retry.textContent = 'Reload page';
    retry.onclick = () => location.reload();
  };
  const timeout = setTimeout(showFailure, 15000);
  import('/viewer.js').then(() => { settled = true; clearTimeout(timeout); }).catch(showFailure);
})();
