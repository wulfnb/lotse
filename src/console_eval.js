let value;
if (/^\s*\(?\s*await\b/.test(source)) {
  const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
  value = await new AsyncFunction('return ('+source+'\n);')();
} else {
  value = await (0,eval)(source);
}
const seen=new WeakSet();
function preview(v,depth){
  if(typeof v==='bigint')return String(v)+'n';
  if(typeof v==='symbol')return String(v);
  if(typeof v==='function')return Function.prototype.toString.call(v).slice(0,8192);
  if(v===null||typeof v!=='object')return v;
  if(seen.has(v))return '[Circular]';
  if(depth>=4)return '[Preview depth limit]';
  seen.add(v);
  const descriptors=Object.getOwnPropertyDescriptors(v),result=Array.isArray(v)?[]:Object.create(null);
  const names=Array.isArray(v)?Object.keys(descriptors).filter(k=>k!=='length'):Object.keys(descriptors);
  for(const name of names.slice(0,100)){
    const descriptor=descriptors[name];
    result[name]='value' in descriptor?preview(descriptor.value,depth+1):'[Accessor — not invoked]';
  }
  if(names.length>100&&!Array.isArray(v))result['[preview]']='Truncated at 100 properties';
  return result;
}
let text=JSON.stringify(preview(value,0),null,2)??String(value);
if(text.length>65536)text=text.slice(0,65536)+'… [preview truncated]';
return {type:typeof value,value:text};
