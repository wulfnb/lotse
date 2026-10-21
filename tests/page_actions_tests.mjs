import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';
const file = name => fileURLToPath(new URL('../src/' + name, import.meta.url));
const find = new vm.Script(readFileSync(file('page_actions.js'), 'utf8'), {filename: file('page_actions.js')});
const reader = new vm.Script(readFileSync(file('reader.js'), 'utf8'), {filename: file('reader.js')});
function textNode(data, options = {}) {
  return {data, parentElement: options.orphan ? null : {closest: () => options.skipped, scrollIntoView() {}, style: {display: options.display || 'block', visibility: options.visibility || 'visible'}}};
}
function findFixture(nodes) {
  const ranges = [];
  const context = vm.createContext({QUERY: '', BACKWARDS: false, NodeFilter: {SHOW_TEXT: 4}, getComputedStyle: parent => parent.style, window: {getSelection: () => ({removeAllRanges() {}, addRange: range => ranges.push(range)})}, document: {
    body: {},
    createTreeWalker: () => { let index = 0; return {nextNode: () => nodes[index++]}; },
    createRange: () => ({setStart(node, offset) {this.start = [node, offset];}, setEnd(node, offset) {this.end = [node, offset];}})
  }});
  return {ranges, run(query, backwards = false) {context.QUERY = query; context.BACKWARDS = backwards; return find.runInContext(context);}};
}

test('find skips hidden and unsafe nodes and preserves text while cycling matches', () => {
  const text = textNode('Needle then NEEDLE');
  const f = findFixture([textNode('needle', {orphan: true}), textNode('needle', {skipped: true}), textNode('needle', {display: 'none'}), textNode('needle', {visibility: 'hidden'}), text]);
  assert.equal(f.run(''), 'Enter text to find');
  assert.equal(f.run('absent'), 'No matches');
  assert.equal(f.run('needle'), '1 of 2');
  assert.equal(f.run('needle'), '2 of 2');
  assert.equal(f.ranges.at(-1).start[1], 12);
  assert.equal(f.run('needle', true), '1 of 2');
  assert.equal(f.run('NEEDLE', true), '2 of 2');
  assert.equal(text.data, 'Needle then NEEDLE');
});

test('find bounds large documents and match counts', () => {
  assert.equal(findFixture([textNode('a'.repeat(10001))]).run('a'), '1 of 10000');
  const nodes = [textNode('hit'), ...Array.from({length: 50001}, () => textNode('miss'))];
  assert.equal(findFixture(nodes).run('hit'), '1 of 1 (search limit reached)');
});

test('reader uses text content and toggles without injecting page HTML', () => {
  for (const useArticle of [true, false]) {
    const ids = new Map();
    const document = {
      title: 'Title <script>unsafe()</script>',
      body: {innerText: 'Body text', appendChild(element) {ids.set(element.id, element);}},
      querySelector: () => useArticle ? {innerText: 'Article text'} : null,
      getElementById: id => ids.get(id),
      createElement: tag => ({tag, style: {}, children: [], appendChild(child) {this.children.push(child);}, remove() {ids.delete(this.id);}})
    };
    const context = vm.createContext({document});
    reader.runInContext(context);
    const panel = ids.get('__servo_reader');
    assert.equal(panel.children[1].textContent, document.title + '\n\n' + (useArticle ? 'Article text' : 'Body text'));
    assert.equal(panel.children[1].innerHTML, undefined);
    panel.children[0].onclick(); assert.equal(ids.size, 0);
    reader.runInContext(context); reader.runInContext(context); assert.equal(ids.size, 0);
  }
});
