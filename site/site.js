const copyButton = document.querySelector('#copy-docker');
const copyStatus = document.querySelector('#copy-status');
copyButton.addEventListener('click', async () => {
  const command = "docker run -d --name wega-browser -p 127.0.0.1:7443:7443 -e BROWSER_USER=servo -e BROWSER_PASSWORD='choose-a-strong-password' -v wega-profile:/config ghcr.io/wulfnb/wega:latest";
  try {
    if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable');
    await navigator.clipboard.writeText(command);
    copyStatus.textContent = 'Docker command copied. Replace the example password before running.';
    copyButton.textContent = 'Copied!';
  } catch {
    copyButton.textContent = 'Copy';
    copyStatus.textContent = 'Could not copy automatically. Select and copy the command below.';
    const range = document.createRange();
    range.selectNodeContents(document.querySelector('#docker-command'));
    const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range);
    document.querySelector('#docker-command').focus();
  }
});
document.querySelectorAll('.mobile-menu a').forEach(link => link.addEventListener('click', () => link.closest('details').removeAttribute('open')));
