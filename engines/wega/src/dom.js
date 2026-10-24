// Wega's initial DOM bindings. Native calls operate on Blitz's single Rust DOM.
(() => {
  const host = (...args) => { const r=JSON.parse(__wega_host(JSON.stringify(args))); if(r.error)throw new Error(r.error); return r.value; };
  const nodes=new Map(), listeners=new Map(), timers=new Map(); let timerId=0;
  const node=id=>{if(id===null)return null;if(!nodes.has(id))nodes.set(id,new Node(id));return nodes.get(id);};
  class Node {
    constructor(id){Object.defineProperty(this,'_id',{value:id});}
    get textContent(){return host('text',this._id);}
    set textContent(value){host('setText',this._id,String(value));}
    get tagName(){return host('tag',this._id);}
    get parentNode(){return node(host('parent',this._id));}
    get childNodes(){return host('children',this._id).map(node);}
    get children(){return this.childNodes.filter(n=>n.tagName);}
    get firstChild(){return this.childNodes[0]||null;}
    getBoundingClientRect(){return host('rect',this._id);}
    get id(){return this.getAttribute('id')||'';}
    set id(value){this.setAttribute('id',value);}
    get className(){return this.getAttribute('class')||'';}
    set className(value){this.setAttribute('class',value);}
    get value(){return this.getAttribute('value')||'';}
    set value(value){this.setAttribute('value',value);}
    getAttribute(name){return host('attr',this._id,String(name));}
    setAttribute(name,value){host('setAttr',this._id,String(name),String(value));}
    removeAttribute(name){host('removeAttr',this._id,String(name));}
    appendChild(child){host('append',this._id,child._id);return child;}
    append(...values){for(const value of values)this.appendChild(value instanceof Node?value:document.createTextNode(String(value)));}
    remove(){host('remove',this._id);}
    addEventListener(type,fn){if(typeof fn!=='function')return;const key=this._id+':'+type;if(!listeners.has(key))listeners.set(key,[]);listeners.get(key).push(fn);}
    removeEventListener(type,fn){const key=this._id+':'+type;listeners.set(key,(listeners.get(key)||[]).filter(f=>f!==fn));}
    get classList(){const element=this;return {add(...names){element.className=[...new Set([...element.className.split(/\s+/).filter(Boolean),...names])].join(' ');},remove(...names){element.className=element.className.split(/\s+/).filter(n=>!names.includes(n)).join(' ');},contains(name){return element.className.split(/\s+/).includes(name);},toggle(name,force){const has=this.contains(name),add=force===undefined?!has:!!force;if(add)this.add(name);else this.remove(name);return add;}};}
  }
  class Event {
    constructor(type){this.type=type;this.defaultPrevented=false;this._stopped=false;}
    preventDefault(){this.defaultPrevented=true;}
    stopPropagation(){this._stopped=true;}
  }
  const document=node(host('root'));
  document.querySelector=selector=>node(host('query',String(selector)));
  document.querySelectorAll=selector=>host('queryAll',String(selector)).map(node);
  document.getElementById=id=>document.querySelector('[id='+JSON.stringify(String(id))+']');
  document.createElement=name=>node(host('create',String(name).toLowerCase()));
  document.createTextNode=text=>node(host('createText',String(text)));
  document.readyState='loading';
  Object.defineProperties(document,{body:{get:()=>document.querySelector('body')},head:{get:()=>document.querySelector('head')},documentElement:{get:()=>document.querySelector('html')},URL:{get:()=>host('url')},title:{get:()=>document.querySelector('title')?.textContent||'',set:value=>{let title=document.querySelector('title');if(!title){title=document.createElement('title');document.head.append(title);}title.textContent=value;}}});
  const dispatch=(type,id)=>{const event=new Event(type);event.target=node(id);for(let current=event.target;current;current=current.parentNode){event.currentTarget=current;const inline=current['on'+type];if(typeof inline==='function' && inline.call(current,event)===false)event.preventDefault();for(const fn of [...(listeners.get(current._id+':'+type)||[])])fn.call(current,event);if(event._stopped)break;}return event.defaultPrevented;};
  const schedule=(fn,delay,repeat,args)=>{if(typeof fn!=='function')throw new TypeError('Timer callback must be a function');if(timers.size>=256)throw new Error('Timer limit exceeded');const id=++timerId;timers.set(id,{fn,args,delay:Math.max(4,Number(delay)||0),due:host('now')+Math.max(4,Number(delay)||0),repeat});return id;};
  Object.assign(globalThis,{Node,Element:Node,Event,document,window:globalThis,self:globalThis,console:{log:(...v)=>host('log',v.map(String).join(' ')),warn:(...v)=>host('log',v.map(String).join(' ')),error:(...v)=>host('log',v.map(String).join(' '))},setTimeout:(fn,delay,...args)=>schedule(fn,delay,false,args),setInterval:(fn,delay,...args)=>schedule(fn,delay,true,args),clearTimeout:id=>timers.delete(id),clearInterval:id=>timers.delete(id),addEventListener:(...args)=>document.addEventListener(...args),removeEventListener:(...args)=>document.removeEventListener(...args)});
  Object.defineProperty(globalThis,'location',{value:{get href(){return host('url');},set href(value){host('navigate',String(value));},assign(value){this.href=value;},replace(value){this.href=value;}}});
  Object.defineProperty(globalThis,'__wega_dispatch',{value:dispatch});
  Object.defineProperty(globalThis,'__wega_timers',{value:()=>{let count=0;for(const [id,timer] of [...timers]){if(timer.due>host('now'))continue;if(timer.repeat)timer.due=host('now')+timer.delay;else timers.delete(id);timer.fn(...timer.args);if(++count>=16)break;}}});
  Object.defineProperty(globalThis,'__wega_next_timer',{value:()=>timers.size?Math.min(...[...timers.values()].map(t=>t.due-host('now'))):Infinity});
})();
