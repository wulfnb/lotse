(() => {
    const old = document.getElementById('__wega_reader');
    if (old) {old.remove(); return;}
    const source = document.querySelector('article,main,[role=main]') || document.body;
    const text = source.innerText;
    const reader = document.createElement('section'); reader.id = '__wega_reader';
    Object.assign(reader.style,{position:'fixed',inset:'0',zIndex:'2147483647',overflow:'auto',background:'#f6f1e6',color:'#24211d',padding:'40px',font:'20px/1.8 Georgia,serif'});
    const close=document.createElement('button'); close.textContent='Exit reader'; close.onclick=()=>reader.remove(); reader.appendChild(close);
    const content=document.createElement('div'); content.style.maxWidth='740px';content.style.margin='24px auto';content.style.whiteSpace='pre-wrap';content.textContent=document.title+'\n\n'+text;reader.appendChild(content);document.body.appendChild(reader);
})()
