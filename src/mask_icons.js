// Scoped MediaWiki Vector fallback while this engine lacks CSS mask-image.
// Use the site's original SVG image, never substitute a guessed icon glyph.
(() => {
  if (CSS.supports('mask-image', 'url(x)') || CSS.supports('-webkit-mask-image', 'url(x)')) return;
  const visited = new Set();
  const scan = async () => {
    for (const link of document.querySelectorAll('link[rel="stylesheet"]')) {
      let url;
      try { url = new URL(link.href, location.href); } catch { continue; }
      if (url.origin !== location.origin || url.pathname !== '/w/load.php' || visited.has(url.href)) continue;
      visited.add(url.href);
      try {
        const response = await fetch(url.href, {cache:'force-cache', credentials:'same-origin'});
        if (!response.ok) continue;
        const css = await response.text();
        if (css.length > 2 * 1024 * 1024) continue;
        const rules = [];
        for (const match of css.matchAll(/([^{}]+)\{([^{}]*mask-image[^{}]*)\}/g)) {
          const selectors = match[1].trim();
          // Restrict overrides to Vector's empty icon elements, preserving the
          // original class/direction/appearance selectors from the stylesheet.
          if (!selectors.split(',').every(selector => /\.vector-icon(?:[.\s:#\[]|$)/.test(selector))) continue;
          const image = match[2].match(/(?:^|;)\s*(?:-webkit-)?mask-image:\s*url\((?:"([^"]*)"|'([^']*)'|([^\s)]+))\)/);
          if (!image) continue;
          const source = new URL(image[1] || image[2] || image[3], url);
          if (source.origin !== location.origin || !['https:', 'http:'].includes(source.protocol)) continue;
          rules.push(`${selectors}{background-image:url(${JSON.stringify(source.href)})!important;background-color:transparent!important;background-size:contain!important;background-position:center!important;background-repeat:no-repeat!important}`);
        }
        if (rules.length) {
          const style = document.createElement('style');
          style.setAttribute('data-wega-mask-fallback', '');
          style.textContent = rules.join('\n');
          document.head.append(style);
        }
      } catch { /* The page remains usable if its optional icon stylesheet fails. */ }
    }
  };
  const start = () => {
    scan();
    // MediaWiki may load its icon module after DOMContentLoaded.
    new MutationObserver(records => { if (records.some(r=>Array.from(r.addedNodes).some(n=>n.tagName === 'LINK'))) scan(); }).observe(document.head,{childList:true});
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, {once:true}); else start();
})();
