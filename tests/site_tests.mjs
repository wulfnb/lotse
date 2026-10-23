import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {fileURLToPath} from 'node:url';
const filename=fileURLToPath(new URL('../site/site.js',import.meta.url));
function fixture(clipboard){
  let copy, menuClick;
  const button={textContent:'Copy',addEventListener:(_,fn)=>{copy=fn;}};
  const notice={textContent:''};let selected=false, focused=false, closed=false;
  const command={focus:()=>{focused=true;}};
  const link={addEventListener:(_,fn)=>{menuClick=fn;},closest:()=>({removeAttribute:()=>{closed=true;}})};
  const context={navigator:{clipboard},document:{querySelector:s=>s==='#copy-docker'?button:s==='#copy-status'?notice:command,querySelectorAll:()=>[link],createRange:()=>({selectNodeContents:node=>assert.equal(node,command)})},window:{getSelection:()=>({removeAllRanges(){},addRange(){selected=true;}})}};
  new vm.Script(readFileSync(filename,'utf8'),{filename}).runInNewContext(context);
  return {button,notice,copy,menuClick,state:()=>({selected,focused,closed})};
}
test('website Copy announces success only after the clipboard operation resolves',async()=>{
  let finish, copied;
  const f=fixture({writeText:text=>{copied=text;return new Promise(resolve=>{finish=resolve;});}});
  const pending=f.copy();assert.equal(f.button.textContent,'Copy');assert.equal(f.notice.textContent,'');
  finish();await pending;
  assert.equal(f.button.textContent,'Copied!');assert.match(copied,/BROWSER_PASSWORD=/);assert.match(copied,/ghcr.io\/wulfnb\/wega:latest$/);
  f.menuClick();assert.equal(f.state().closed,true);
});
for(const [name,clipboard] of [['unavailable',undefined],['denied',{writeText:async()=>{throw Error('denied');}}]]){
 test('website Copy selects readable fallback when '+name,async()=>{
  const f=fixture(clipboard);await f.copy();assert.equal(f.button.textContent,'Copy');
  assert.match(f.notice.textContent,/Could not copy/);assert.equal(f.state().focused,true);assert.equal(f.state().selected,true);
 });
}
