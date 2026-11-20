import {createGestureInput} from '/gesture-input.mjs';
const form=document.querySelector('#pair'),secretInput=document.querySelector('#secret');
const surface=document.querySelector('#surface'),status=document.querySelector('#status'),layer=document.querySelector('#layer'),disconnect=document.querySelector('#disconnect');
let secret='',queue=[],inFlight=false,enabled=false,timer=null;
function fail(message){enabled=false;queue=[];secret='';status.textContent=message;}
async function flush(){
  timer=null;
  if(!enabled || inFlight || queue.length===0)return;
  inFlight=true;
  const batch=queue.splice(0,32);
  try {
    const response=await fetch('/gestures',{method:'POST',headers:{'Content-Type':'application/json','Authorization':`Bearer ${secret}`},body:JSON.stringify(batch),cache:'no-store'});
    if(!response.ok)throw new Error(`HTTP ${response.status}`);
    status.textContent='Connected — samples delivered to laptop';
  } catch(error){fail(`Delivery stopped (${error.message}). Reconnect before continuing.`);}
  finally {inFlight=false;if(enabled && queue.length)timer=setTimeout(flush,0);}
}
function enqueue(sample){
  if(!enabled)return;
  if(queue.length>=256){fail('Delivery stopped: input queue exceeded capacity. Reconnect before continuing.');return;}
  queue.push(sample);
  if(!timer && !inFlight)timer=setTimeout(flush,16);
}
form.addEventListener('submit',async event=>{
  event.preventDefault();
  const value=secretInput.value.trim();secretInput.value='';
  if(!/^[A-Za-z0-9_-]{43}$/.test(value)){status.textContent='Invalid session secret';return;}
  if(enabled){status.textContent='Disconnect before pairing again';return;}
  const button=form.querySelector('button');button.disabled=true;
  try {
    const response=await fetch('/pair',{method:'POST',headers:{'Content-Type':'application/json','Authorization':`Bearer ${value}`},cache:'no-store'});
    if(!response.ok)throw new Error(`HTTP ${response.status}`);
    const pair=await response.json();secret=pair.token;queue=[];enabled=true;status.textContent='Connected — draw on the surface';
  }catch(error){fail(`Pairing failed (${error.message}).`);}finally{button.disabled=false;}
});
disconnect.addEventListener('click',async()=>{
  const previous=secret;enabled=false;queue=[];secret='';
  if(previous)try{await fetch('/unpair',{method:'POST',headers:{'Content-Type':'application/json','Authorization':`Bearer ${previous}`},cache:'no-store'});}catch{}
  status.textContent='Disconnected';
});
createGestureInput(surface,{onGesture:enqueue,getLayerIds:()=>layer.value.split(',').map(s=>s.trim()).filter(Boolean)});
