// Operates on DOM ranges, preserving the site's markup and event handlers.
((query, backwards) => {
    if (!query) return 'Enter text to find';
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const needle = query.toLocaleLowerCase(), matches = [];
    let node, count = 0;
    while ((node = walker.nextNode()) && ++count < 50000 && matches.length < 10000) {
        const parent = node.parentElement;
        if (!parent || parent.closest('script,style,noscript,textarea,input,[hidden]')) continue;
        const style = getComputedStyle(parent);
        if (style.display === 'none' || style.visibility === 'hidden') continue;
        const text = node.data.toLocaleLowerCase();
        let offset = 0, at;
        while ((at = text.indexOf(needle, offset)) !== -1 && matches.length < 10000) {
            matches.push([node, at]); offset = at + Math.max(1, needle.length);
        }
    }
    if (!matches.length) return 'No matches';
    const last = window.__servoBrowserFind;
    const index = last && last.query === query ? (last.index + (backwards ? -1 : 1) + matches.length) % matches.length : (backwards ? matches.length - 1 : 0);
    const [text, offset] = matches[index], range = document.createRange();
    range.setStart(text, offset); range.setEnd(text, offset + query.length);
    const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range);
    text.parentElement.scrollIntoView({block:'center'});
    window.__servoBrowserFind = {query,index};
    return (index + 1) + ' of ' + matches.length + (count >= 50000 ? ' (search limit reached)' : '');
})(QUERY, BACKWARDS)
