import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';

function snippet(name, parameters, async = false) {
  const filename = fileURLToPath(new URL('../src/' + name, import.meta.url));
  const body = readFileSync(filename, 'utf8').trimEnd();
  // These files are function bodies in Servo, too. Put the harness delimiters on
  // the existing first/last lines so all coverage line numbers stay source exact.
  return new vm.Script(`(${async ? 'async ' : ''}function(${parameters}) {${body}})`, {filename});
}
const evaluate = snippet('console_eval.js', 'source', true).runInNewContext();

test('console previews await expressions and primitive values without invoking accessors', async () => {
  assert.equal((await evaluate('await Promise.resolve(7)')).value, '7');
  assert.equal((await evaluate('undefined')).value, 'undefined');
  assert.equal((await evaluate('12n')).value, '"12n"');
  assert.equal((await evaluate('Symbol("x")')).value, '"Symbol(x)"');
  assert.ok((await evaluate('(function example(){})')).value.includes('function example'));
  const accessor = await evaluate('Object.defineProperty({}, "secret", {enumerable:true,get(){throw Error("must not run")}})');
  assert.ok(accessor.value.includes('Accessor — not invoked'));
  assert.ok((await evaluate('(()=>{const v={};v.self=v;return v;})()')).value.includes('[Circular]'));
  assert.ok((await evaluate('({next:{next:{next:{next:{next:{value:1}}}}}})')).value.includes('Preview depth limit'));
  const array = JSON.parse((await evaluate('[1,null,{value:true}]')).value);
  assert.deepEqual(array, [1, null, {value:true}]);
  assert.ok((await evaluate('Object.fromEntries(Array.from({length:105},(_,i)=>["p"+i,i]))')).value.includes('Truncated at 100 properties'));
  assert.equal(JSON.parse((await evaluate('Array.from({length:105},(_,i)=>i)')).value).length, 100);
  assert.ok((await evaluate('"x".repeat(70000)')).value.endsWith('… [preview truncated]'));
  await assert.rejects(evaluate('throw new Error("explicit evaluation error")'), /explicit evaluation error/);
});

const inspect = snippet('inspector.js', 'selector');
function element(tag, options={}) {
  return {nodeType:1, localName:tag, id:options.id||'', className:options.className||'', parentElement:null, previousElementSibling:null,
    children:[], childNodes:[{nodeType:3,textContent:' visible text '},{nodeType:1}], attributes:[{name:'class',value:'example'}],
    outerHTML:options.large ? 'x'.repeat(70000) : '<'+tag+'>text</'+tag+'>', style:{cssText:'color:red'},
    getAttribute:()=>options.className||'', matches: selector=>{if(selector==='invalid[')throw Error('invalid');return selector==='.match';},
    getBoundingClientRect:()=>({x:1,y:2,width:100,height:30})};
}
function inspectorFixture({selected=true, fallbackNames=false, large=false, many=false}={}) {
  const html=element('html'), chosen=element('main',{id:'selected',className:'example',large}), child=element('span',{className:{baseVal:'svg-class'}});
  chosen.parentElement=html; child.parentElement=chosen; child.previousElementSibling=element('a'); chosen.children=[child]; html.children=[chosen];
  const style={display:'grid',boxSizing:'border-box',getPropertyValue:name=>name==='color'?'red':'0px',*[Symbol.iterator](){yield* (fallbackNames?[]:['color','display']);}};
  const rule=selectorText=>({selectorText,style:{cssText:'color: red'}});
  const mediaSkip={cssRules:[rule('.match')],media:{},conditionText:'skip',constructor:{name:'CSSMediaRule'}};
  const supportsSkip={cssRules:[rule('.match')],conditionText:'unsupported',constructor:{name:'CSSSupportsRule'}};
  const nested={cssRules:[rule('.match')],cssText:'@media screen {x}',constructor:{name:'CSSMediaRule'}};
  const inaccessibleImport={href:'https://cross.example/a.css',styleSheet:{get cssRules(){throw Error('CORS');}}};
  const sheet=cssRules=>({disabled:false,media:{length:0},cssRules});
  const sheets=[{disabled:true}, {disabled:false,media:{length:1,mediaText:'skip'}}, sheet([rule('.match'),rule('.other'),rule('invalid['),mediaSkip,supportsSkip,nested,inaccessibleImport,{styleSheet:{cssRules:[rule('.match')]}}]), {disabled:false,media:{length:0},get cssRules(){throw Error('CORS');}}, sheet(Array.from({length:110},()=>rule('.match'))), sheet(Array.from({length:3005},()=>({})))];
  let index=0;
  const context=vm.createContext({document:{documentElement:html,querySelector:()=>selected?chosen:null,querySelectorAll:selector=>selector==='#selected'?[chosen]:[],styleSheets:sheets,createTreeWalker:()=>({nextNode:()=>many?chosen:[chosen,child][index++]})},CSS:{escape:value=>value,supports:()=>false},NodeFilter:{SHOW_ELEMENT:1},getComputedStyle:()=>style,matchMedia:query=>({matches:query!=='skip'}),location:{href:'https://example.com'},innerWidth:1000,innerHeight:700,__servoInspectorChanges:[{description:'changed'}]});
  return inspect.runInContext(context)('main');
}

test('inspector reads DOM, computed styles and rules while isolating CORS/selector errors', () => {
  const result=inspectorFixture();
  assert.equal(result.nodes.length,3);assert.equal(result.nodes[2].classes,'');
  assert.equal(result.selected.box.width,100);assert.equal(result.selected.styles.color,'red');
  assert.equal(result.selected.rules.length,100);assert.equal(result.selected.rulesTruncated,true);
  assert.ok(result.selected.inaccessible.length>=2);assert.equal(result.selected.layoutItems.length,1);
  assert.equal(result.changes[0],'changed');
  const fallback=inspectorFixture({fallbackNames:true,large:true});
  assert.equal(fallback.selected.html.length,65536);assert.equal(fallback.selected.htmlTruncated,true);
  assert.ok(fallback.selected.computed.length>=10);
  assert.equal(inspectorFixture({selected:false}).selected,null);
  assert.equal(inspectorFixture({many:true}).nodes.length,500);
  assert.equal(inspectorFixture({many:true}).truncated,true);
});
