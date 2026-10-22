window.advancedCalls = 0;
window.advancedToolsProbe = function advancedToolsProbe() {
  const outerSecret = 40;
  function nestedFrame() {
    const localSecret = 2;
    window.advancedCalls += outerSecret + localSecret;
    return outerSecret + localSecret;
  }
  return nestedFrame();
};
window.advancedLogs = function advancedLogs() {
  console.info('advanced-info', { answer:42, nan:NaN, nested:{name:'native preview'}, items:[1,2,3] });
  for(let i=0;i<2;i++) console.warn('advanced-warning');
  console.error('advanced-error');
  console.trace('advanced-trace');
  console.log('advanced-numbers', NaN, Infinity, undefined, null);
  console.log('preview-types', false, -0, new Map([[Infinity,-Infinity]]), { nested: new Set([NaN]) });
};
