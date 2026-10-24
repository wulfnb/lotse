// Element-local feedback for real media failures, including unbundled/unsupported codecs.
(() => {
  const panels = new WeakMap();
  document.addEventListener('error', event => {
    const media = event.target;
    if (!['VIDEO','AUDIO'].includes(media?.tagName) || !media.error || media.error.code === 1) return;
    panels.get(media)?.remove();
    const panel = document.createElement('div');
    panel.setAttribute('data-wega-media-message', '');
    panel.setAttribute('role', 'status');
    panel.style.cssText = 'font:14px system-ui;padding:12px;margin:8px 0;border:1px solid currentColor;border-radius:8px;max-width:38rem';
    const message = document.createElement('p');
    message.textContent = media.error.code === 2 ? 'The media could not be downloaded. Check your connection or try again.' : 'This media could not be played. Its format may be unsupported, or the file may be damaged.';
    panel.append(message);
    try {
      const url = new URL(media.currentSrc || media.src || 'about:blank', location.href);
      if (['http:', 'https:'].includes(url.protocol)) {
        const download = document.createElement('a');
        download.href = url.href;
        download.setAttribute('download', '');
        download.textContent = 'Download media to play in another app';
        panel.append(download);
      }
    } catch { /* A failed media source need not be a valid URL. */ }
    media.after(panel); panels.set(media, panel);
  }, true);
  for (const event of ['loadstart', 'loadeddata']) {
    document.addEventListener(event, e => { panels.get(e.target)?.remove(); panels.delete(e.target); }, true);
  }
})();
