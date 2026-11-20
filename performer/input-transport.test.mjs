// Synthetic DOM events through the shipped Go TLS process and editor mapping.
// Run with TSS_PERFORMER_BIN set to the matching local executable.
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync,spawn} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import https from 'node:https';
import {once} from 'node:events';
import {createGestureInput} from './surface/gesture-input.mjs';
import {PerformerRoutes} from '../WebAssets/performer-host.js';
class Surface extends EventTarget {
  ownerDocument={defaultView:{MouseEvent:{WEBKIT_FORCE_AT_MOUSE_DOWN:1,WEBKIT_FORCE_AT_FORCE_MOUSE_DOWN:2}}};
  getBoundingClientRect(){return{left:0,top:0,width:200,height:100};}
  emit(type,properties){this.dispatchEvent(Object.assign(new Event(type,{cancelable:true}),properties));}
}
test('paired trackpad force and Pencil reach separate native slide commands through TLS',{timeout:20000},async()=>{
  assert.ok(process.env.TSS_PERFORMER_BIN,'Set TSS_PERFORMER_BIN');
  const dir=await mkdtemp(join(tmpdir(),'tss-performance-proof-'));
  const cert=join(dir,'cert.pem'),key=join(dir,'key.pem');
  execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',key,'-out',cert,'-days','1','-subj','/CN=localhost','-addext','subjectAltName=IP:127.0.0.1'],{stdio:'ignore'});
  const ca=await readFile(cert);
  const child=spawn(process.env.TSS_PERFORMER_BIN,[],{stdio:['pipe','pipe','inherit']});const exit=once(child,'exit');
  const lines=createInterface({input:child.stdout})[Symbol.asyncIterator]();const line=async()=>JSON.parse((await lines.next()).value);
  try{
    child.stdin.write(JSON.stringify({bind:'127.0.0.1',host:'127.0.0.1',port:0,cert,key})+'\n');
    const ready=await line();assert.equal(ready.kind,'ready');
    const post=(path,secret,data)=>new Promise((resolve,reject)=>{
      const r=https.request(new URL('/'+path,ready.url),{method:'POST',ca,headers:{Origin:ready.url,Authorization:'Bearer '+secret,'Content-Type':'application/json'}},res=>{
        const chunks=[];res.on('data',c=>chunks.push(c));res.on('end',()=>{const body=Buffer.concat(chunks).toString();if(res.statusCode>=300)return reject(Error(body));
          resolve(body?JSON.parse(body):null);});});r.on('error',reject);r.end(JSON.stringify(data));
    });
    const pad=await post('pair',ready.secret,{name:'trackpad'}),pen=await post('pair',ready.secret,{name:'pencil'});
    assert.equal((await line()).source,pad.source);assert.equal((await line()).source,pen.source);
    let selected='rear';const outputs=[];
    const router=new PerformerRoutes({mapping:s=>s===pad.source?{layerId:3,slide:selected,mode:'auto',response:.4}:{layerId:5,slide:'front',mode:'auto',response:.4},
      invoke:async(name,payload)=>outputs.push({name,payload})});
    async function sample(source,token,sequence,samples){
      await post('gestures',token,{sequence,samples});const e=await line();assert.equal(e.source,source);await router.batch(e.source,e.samples);
    }
    const a=new Surface(),b=new Surface(),aa=[],bb=[];const A=createGestureInput(a,{onGesture:g=>aa.push(g)}),B=createGestureInput(b,{onGesture:g=>bb.push(g)});
    const mouse={pointerType:'mouse',pointerId:1,pressure:.5,clientX:100,clientY:50,button:0};
    a.emit('pointerdown',mouse);await sample(pad.source,pad.token,1,aa.splice(0));assert.equal(outputs.length,0,'synthetic mouse pressure must not actuate');
    selected='front';a.emit('webkitmouseforcechanged',{webkitForce:1.75,clientX:100,clientY:50});await sample(pad.source,pad.token,2,aa.splice(0));
    assert.equal(outputs[0].name,'set_live_wheel_pressure');assert.equal(outputs[0].payload.amount,.75);
    assert.equal(outputs[0].payload.layerId,3);assert.equal(outputs[0].payload.slide,'rear','mapping must stay pinned during stroke');
    const p={...mouse,pointerType:'pen',pressure:.4,tiltX:15,tiltY:-20};b.emit('pointerdown',p);b.emit('pointermove',{...p,clientX:140});b.emit('pointerup',p);
    await sample(pen.source,pen.token,1,bb.splice(0));assert.equal(outputs[1].name,'perform_live_wheel_gesture');
    const g=outputs[1].payload.batch;assert.equal(g.layerId,5);assert.equal(g.slide,'front');assert.equal(g.gesture,'tug');assert.deepEqual(g.samples.map(v=>v.phase),['down','move','up']);
    await post('unpair',pad.token,{});const left=await line();assert.equal(left.kind,'left');await router.leave(left.source);
    assert.equal(outputs.at(-1).payload.amount,0);assert.equal(outputs.at(-1).payload.phase,'up');A.dispose();B.dispose();
  }finally{child.stdin.end();await exit;await rm(dir,{recursive:true,force:true});}
});
