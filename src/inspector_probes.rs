//! Page mutations are explicit user actions; only our attribute edits are undoable.
use crate::{probes::INSPECTOR, toolbox::quote};
pub fn attribute(selector: &str, name: &str, value: Option<&str>) -> String {
    format!(
        r#"let selector={};const e=document.querySelector(selector);if(!e)throw Error('No matching element');
const name={};const before=e.getAttribute(name);const value={};
if(value===null)e.removeAttribute(name);else e.setAttribute(name,value);selector=path(e);
globalThis.__wegaInspectorChanges ||= [];__wegaInspectorChanges.push({{element:e,name,before,description:selector+' · '+name+' = '+String(value)}});if(__wegaInspectorChanges.length>100)__wegaInspectorChanges.shift();
{}"#,
        quote(selector),
        quote(name),
        value.map(quote).unwrap_or("null".into()),
        INSPECTOR
    )
}
pub fn undo(selector: &str) -> String {
    format!(
        r#"let selector={};const change=globalThis.__wegaInspectorChanges?.pop();
if(change){{if(!change.element.isConnected)throw Error('Changed element no longer exists');if(change.before===null)change.element.removeAttribute(change.name);else change.element.setAttribute(change.name,change.before);selector=path(change.element);}}
{}"#,
        quote(selector),
        INSPECTOR
    )
}
pub fn picker() -> String {
    // All listeners and temporary outlines are removed on success, Escape, or timeout.
    format!(
        r#"if(globalThis.__wegaPickerCancel)globalThis.__wegaPickerCancel();
let selector=await new Promise((resolve,reject)=>{{
let hovered=null,outline='',priority='',hadStyle=false;
function restore(){{if(hovered){{hovered.style.setProperty('outline',outline,priority);if(!outline)hovered.style.removeProperty('outline');if(!hadStyle&&!hovered.style.length)hovered.removeAttribute('style');hovered=null;}}}}
function move(event){{restore();if(event.target instanceof Element){{hovered=event.target;hadStyle=hovered.hasAttribute('style');outline=hovered.style.getPropertyValue('outline');priority=hovered.style.getPropertyPriority('outline');hovered.style.setProperty('outline','2px solid #b899ff','important');}}}}
function cleanup(){{clearTimeout(timer);restore();document.removeEventListener('mousemove',move,true);document.removeEventListener('click',click,true);document.removeEventListener('keydown',key,true);delete globalThis.__wegaPickerCancel;}}
function cancel(){{cleanup();reject(Error('Element picker cancelled'));}}
function click(event){{if(!(event.target instanceof Element))return;event.preventDefault();event.stopImmediatePropagation();const target=event.target;cleanup();const parts=[];let e=target;if(e.id&&document.querySelectorAll('#'+CSS.escape(e.id)).length===1){{resolve('#'+CSS.escape(e.id));return;}}while(e&&e.nodeType===1){{let n=1,p=e;while((p=p.previousElementSibling))n++;parts.unshift(e.localName+':nth-child('+n+')');e=e.parentElement;}}resolve(parts.join(' > '));}}
function key(event){{if(event.key==='Escape'){{event.preventDefault();event.stopImmediatePropagation();cancel();}}}}
const timer=setTimeout(cancel,9000);globalThis.__wegaPickerCancel=cancel;
document.addEventListener('mousemove',move,true);document.addEventListener('click',click,true);document.addEventListener('keydown',key,true);
}});
{}"#,
        INSPECTOR
    )
}
