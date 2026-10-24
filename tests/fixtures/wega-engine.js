const counter=document.getElementById('counter');
counter.addEventListener('click',()=>{counter.textContent=String(Number(counter.textContent)+1);document.title='Counter clicked: '+counter.textContent;});
document.addEventListener('DOMContentLoaded',()=>{document.getElementById('ready').textContent='JavaScript is connected to the Rust DOM.';document.title='Wega engine ready';});
