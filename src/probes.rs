//! Page-side probes used only when the user opens a developer tool.
pub const INSPECTOR: &str = include_str!("inspector.js");
pub const PERFORMANCE: &str = r#"
const c=globalThis.__wegaPerfRecording;
let raw=performance.getEntries();
if(c){raw=[...raw,...c.entries];raw=raw.filter(e=>e.startTime>=c.start);}
const seen=new Set();raw=raw.filter(e=>{const k=JSON.stringify([e.entryType,e.name,e.startTime,e.duration]);if(seen.has(k))return false;seen.add(k);return true;});
function rows(type){return raw.filter(e=>e.entryType===type).slice(-2000).map(e=>({name:e.name,type:e.type,initiator:e.initiatorType,start:e.startTime,duration:e.duration,transferSize:e.transferSize,decodedBodySize:e.decodedBodySize,requestStart:e.requestStart,responseStart:e.responseStart,responseEnd:e.responseEnd,domInteractive:e.domInteractive,domContentLoaded:e.domContentLoadedEventEnd,load:e.loadEventEnd}));}
return {timeOrigin:performance.timeOrigin,now:performance.now(),navigation:rows('navigation'),resources:rows('resource'),marks:rows('mark'),measures:rows('measure'),paint:rows('paint'),longtasks:rows('longtask'),recording:c?{running:c.running,start:c.start,end:c.end??performance.now(),frames:c.frames,truncated:c.truncated,observerError:c.observerError}:null};
"#;
pub const STORAGE: &str = r#"
function read(name){try{const store=window[name];return Object.keys(store).sort().map(key=>({key,value:store.getItem(key)}));}catch(error){return {error:String(error)};}}
let cookies=null,cookieError=null;try{cookies=document.cookie;}catch(error){cookieError=String(error);}
return {origin:location.origin,url:location.href,opaque:location.origin==='null',local:read('localStorage'),session:read('sessionStorage'),cookies,cookieError};
"#;
pub const APPLICATION: &str = r#"
const link=document.querySelector('link[rel~="manifest"]');let manifest=null,error=null;
if(link){try{const response=await fetch(link.href);if(!response.ok)throw Error('HTTP '+response.status);manifest=await response.json();}catch(e){error=String(e);}}
let workers=[],workerError=null;try{if(navigator.serviceWorker){workers=(await navigator.serviceWorker.getRegistrations()).map(r=>({scope:r.scope,active:r.active&&{scriptURL:r.active.scriptURL,state:r.active.state},waiting:r.waiting&&{scriptURL:r.waiting.scriptURL,state:r.waiting.state},installing:r.installing&&{scriptURL:r.installing.scriptURL,state:r.installing.state}}));}}catch(e){workerError=String(e);}
let cacheNames=[];try{if('caches' in window)cacheNames=await caches.keys();}catch(e){}
return {url:location.href,manifestURL:link?link.href:null,manifest,error,serviceWorkerSupported:!!navigator.serviceWorker,workers,workerError,cacheNames,secureContext:window.isSecureContext};
"#;
pub const ACCESSIBILITY: &str = r#"
return {title:document.title,language:document.documentElement.lang,landmarks:Array.from(document.querySelectorAll('main,nav,header,footer,aside,[role]')).slice(0,100).map(e=>({tag:e.localName,role:e.getAttribute('role'),name:e.getAttribute('aria-label')})),imagesWithoutAlt:Array.from(document.images).filter(e=>!e.hasAttribute('alt')).map(e=>e.currentSrc),unnamedButtons:Array.from(document.querySelectorAll('button,[role=button]')).filter(e=>!(e.textContent.trim()||e.getAttribute('aria-label')||e.getAttribute('aria-labelledby')||e.getAttribute('title'))).length};
"#;

pub fn highlight(selector: &str) -> String {
    format!(
        "const selector={};const e=document.querySelector(selector);if(!e)throw Error('No matching element');e.scrollIntoView({{block:'center'}});const old=e.style.outline;e.style.outline='2px solid #b899ff';setTimeout(()=>e.style.outline=old,1500);{}",
        crate::toolbox::quote(selector),
        INSPECTOR
    )
}
pub fn edit_html(selector: &str, html: &str) -> String {
    format!(
        "const selector={};const e=document.querySelector(selector);if(!e)throw Error('No matching element');e.outerHTML={};{}",
        crate::toolbox::quote(selector),
        crate::toolbox::quote(html),
        INSPECTOR
    )
}
pub fn edit_css(index: i32, css: &str) -> String {
    format!(
        r#"const index={index};
const original=document.styleSheets[index];
globalThis.__wegaEditorSheets ||= new Map();
if(original){{if(!__wegaEditorSheets.has(original))__wegaEditorSheets.set(original,original.disabled);original.disabled=true;}}
let style=document.querySelector('style[data-wega-editor="'+index+'"]');
if(!style){{style=document.createElement('style');style.dataset.wegaEditor=String(index);document.head.append(style);}}
style.textContent={};return style.textContent;"#,
        crate::toolbox::quote(css)
    )
}
pub const RESET_CSS: &str = r#"
document.querySelectorAll('style[data-wega-editor]').forEach(e=>e.remove());
if(globalThis.__wegaEditorSheets){for(const [sheet,disabled] of __wegaEditorSheets)sheet.disabled=disabled;__wegaEditorSheets.clear();}
return '';
"#;
