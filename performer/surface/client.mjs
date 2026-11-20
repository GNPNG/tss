import {createGestureInput} from './gesture-input.mjs';

const form=document.querySelector('#pair'), status=document.querySelector('#connection');
const pad=document.querySelector('#pad'), contact=document.querySelector('#contact'), force=document.querySelector('#force');
const disconnect=document.querySelector('#disconnect'), instruction=document.querySelector('#instruction');
if(location.hash){form.elements.secret.value=location.hash.slice(1);history.replaceState(null,'',location.pathname);}
let session=null;
function post(route,key,data,extra={}) {
  return fetch('/'+route,{method:'POST',cache:'no-store',headers:{'Content-Type':'application/json',Authorization:'Bearer '+key},body:JSON.stringify(data),...extra});
}
function stop(s,message) {
  if(session!==s)return;
  session=null;clearInterval(s.pulse);clearTimeout(s.timer);
  void post('unpair',s.key,{}, {keepalive:true}).catch(()=>{});
  s.input.dispose();form.hidden=false;contact.hidden=true;disconnect.disabled=true;
  status.textContent=message;instruction.textContent='Pair with the editor to perform.';force.textContent='Pressure —';
}
async function flush(s){
  if(session!==s || s.sending || !s.queue.length)return;
  s.sending=true;
  try{
    while(session===s && s.queue.length){
      const samples=s.queue.splice(0,32);
      const res=await post('gestures',s.key,{sequence:++s.sequence,samples});
      if(!res.ok)throw Error('Input rejected ('+res.status+')');
    }
  }catch(e){stop(s,String(e));}finally{s.sending=false;}
}
function receive(s,g){
  if(session!==s)return;
  if(s.queue.length>=256){stop(s,'Input queue full. Pair again.');return;}
  contact.hidden=g.phase==='up'||g.phase==='cancel';
  contact.style.left=(g.x*100)+'%';contact.style.top=(g.y*100)+'%';
  force.textContent=g.pressure===null?'Pressure —':'Pressure '+Math.round(g.pressure*100)+'%';
  s.queue.push(g);
  if(!s.sending && !s.timer)s.timer=setTimeout(()=>{s.timer=null;void flush(s);},8);
}
form.addEventListener('submit',async e=>{
  e.preventDefault();if(session)return;
  const button=form.querySelector('button');button.disabled=true;
  try{
    const res=await post('pair',form.elements.secret.value.trim(),{name:form.elements.name.value.trim()});
    if(!res.ok)throw Error('Pairing rejected ('+res.status+')');
    const pair=await res.json();const s={key:pair.token,sequence:0,sending:false,queue:[],timer:null};
    session=s;s.input=createGestureInput(pad,{onGesture:g=>receive(s,g)});
    s.pulse=setInterval(async()=>{try{const r=await post('pulse',s.key,{});if(!r.ok)stop(s,'Session expired');}catch{stop(s,'Connection lost');}},3000);
    form.elements.secret.value='';form.hidden=true;disconnect.disabled=false;status.textContent='Connected as '+form.elements.name.value;
    instruction.textContent='Draw with Pencil or press a Force Touch trackpad. The editor controls each device’s assignment.';
  }catch(err){status.textContent=String(err);}finally{button.disabled=false;}
});
disconnect.addEventListener('click',()=>session&&stop(session,'Disconnected'));
addEventListener('pagehide',()=>session&&stop(session,'Disconnected'));
