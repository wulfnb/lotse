function path(e) {
  if (e.id && document.querySelectorAll('#'+CSS.escape(e.id)).length===1) return '#'+CSS.escape(e.id);
  const parts=[];
  while(e && e.nodeType===1) { let n=1,p=e;while((p=p.previousElementSibling))n++;parts.unshift(e.localName+':nth-child('+n+')');e=e.parentElement; }
  return parts.join(' > ');
}
const chosen=document.querySelector(selector);
const nodes=[];const walker=document.createTreeWalker(document.documentElement,NodeFilter.SHOW_ELEMENT);let node=document.documentElement;
while(node && nodes.length<500) {
  let depth=0,p=node;while(p.parentElement){depth++;p=p.parentElement;}
  nodes.push({tag:node.localName,id:node.id,classes:typeof node.className==='string'?node.className:'',selector:path(node),depth,children:node.children.length,text:Array.from(node.childNodes).filter(n=>n.nodeType===3).map(n=>n.textContent).join(' ').trim().slice(0,120)});
  node=walker.nextNode();
}
let selected=null;
if(chosen) {
  const s=getComputedStyle(chosen),r=chosen.getBoundingClientRect();
  const names=Array.from(s);if(!names.length)names.push('display','position','width','height','color','background-color','font-family','font-size','margin','padding');
  const styles=Object.fromEntries(['display','position','width','height','margin','padding','border','color','background-color','font-family','font-size','line-height','overflow','z-index'].map(k=>[k,s.getPropertyValue(k)]));
  const boxSides=prefix=>Object.fromEntries(['top','right','bottom','left'].map(side=>[side,s.getPropertyValue(prefix==='border'?'border-'+side+'-width':prefix+'-'+side)]));
  const rules=[],inaccessible=[];let visited=0;
  function walk(list,source,context) {
    for(const rule of list) {
      if(++visited>3000 || rules.length>=100)return;
      if(rule.selectorText) { try { if(chosen.matches(rule.selectorText)) rules.push({selector:rule.selectorText,css:rule.style.cssText.slice(0,8192),source,context}); }catch(e){} }
      else if(rule.styleSheet) { try{walk(rule.styleSheet.cssRules,rule.href||source,context);}catch(e){inaccessible.push(rule.href||source);} }
      else if(rule.cssRules) {
        if(rule.media && !matchMedia(rule.conditionText).matches)continue;
        if(rule.constructor.name==='CSSSupportsRule' && !CSS.supports(rule.conditionText))continue;
        walk(rule.cssRules,source,context.concat((rule.cssText||'').split('{')[0].trim()).slice(-6));
      }
    }
  }
  for(const sheet of document.styleSheets) {
    if(sheet.disabled || (sheet.media.length && !matchMedia(sheet.media.mediaText).matches))continue;
    try{walk(sheet.cssRules,sheet.href||'Inline stylesheet',[]);}catch(e){inaccessible.push(sheet.href||'Inline stylesheet');}
  }
  const layout=Object.fromEntries(['display','position','box-sizing','flex-direction','flex-wrap','justify-content','align-items','align-content','gap','row-gap','column-gap','grid-template-columns','grid-template-rows','grid-auto-flow','flex','align-self','grid-column','grid-row'].map(k=>[k,s.getPropertyValue(k)]));
  selected={selector,html:chosen.outerHTML.slice(0,65536),htmlTruncated:chosen.outerHTML.length>65536,attributes:Array.from(chosen.attributes).map(a=>({name:a.name,value:a.value})),classes:chosen.getAttribute('class')||'',inlineCSS:chosen.style.cssText,
    box:{x:r.x,y:r.y,width:r.width,height:r.height,margin:boxSides('margin'),border:boxSides('border'),padding:boxSides('padding'),sizing:s.boxSizing},styles,
    computed:names.slice(0,512).map(name=>({name,value:s.getPropertyValue(name)})),rules,inaccessible,rulesTruncated:visited>3000||rules.length>=100,layout,
    layoutItems:/flex|grid/.test(s.display)?Array.from(chosen.children).slice(0,100).map(e=>({selector:path(e),tag:e.localName})):[],
    ancestors:(()=>{let e=chosen;const list=[];while(e){list.unshift({tag:e.localName,selector:path(e)});e=e.parentElement;}return list;})()};
}
return {url:location.href,viewport:{width:innerWidth,height:innerHeight},nodes,selected,truncated:!!node,changes:(globalThis.__servoInspectorChanges||[]).map(c=>c.description)};
