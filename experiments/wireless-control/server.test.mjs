import {test, before, after} from 'node:test';
import assert from 'node:assert/strict';
import https from 'node:https';
import {execFileSync} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {randomBytes} from 'node:crypto';
import {startController} from './server.mjs';
let directory, app, cert, token, received=[];
const sample={time:1.25,pointerId:9,pointerType:'pen',phase:'move',x:0.3,y:0.7,pressure:0.8,pressureSource:'pointer',rawForce:null,tiltX:10,tiltY:-15,layerIds:Array.from({length:30},(_,i)=>String(i))};
function request(path,{method='GET',body,auth=token,origin=app.origin,host,ca=cert,contentType='application/json'}={}){
  return new Promise((resolve,reject)=>{
    const url=new URL(path,app.origin);
    const headers={}; if(method==='POST'){if(origin!==null)headers.Origin=origin;if(auth!==null)headers.Authorization=`Bearer ${auth}`;headers['Content-Type']=contentType;}
    if(host)headers.Host=host;
    const req=https.request(url,{method,headers,ca,servername:'localhost',timeout:3000},res=>{const chunks=[];res.on('data',c=>chunks.push(c));res.on('end',()=>resolve({status:res.statusCode,body:Buffer.concat(chunks).toString(),headers:res.headers}));});
    req.on('timeout',()=>req.destroy(new Error('timeout')));req.on('error',reject);if(body!==undefined)req.write(body);req.end();
  });
}
async function pair(){const response=await request('/pair',{method:'POST'});assert.equal(response.status,200);return JSON.parse(response.body);}
before(async()=>{
  directory=await mkdtemp(join(tmpdir(),'tss-wireless-test-'));
  const keyPath=join(directory,'key.pem'),certPath=join(directory,'cert.pem');
  execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',keyPath,'-out',certPath,'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'],{stdio:'ignore'});
  cert=await readFile(certPath);token=randomBytes(32).toString('base64url');
  app=await startController({key:await readFile(keyPath),cert,bindHost:'127.0.0.1',port:0,publicHost:'localhost',token,ratePerSecond:500,onGestures:batch=>received.push(...batch)});
});
after(async()=>{if(app)await app.close();if(directory)await rm(directory,{recursive:true,force:true});});
test('trusted HTTPS serves only declared assets with restrictive headers',async()=>{
  const response=await request('/');assert.equal(response.status,200);assert.match(response.headers['content-security-policy'],/frame-ancestors 'none'/);
  assert.equal((await request('/gesture-input.mjs')).status,200);
  assert.equal((await request('/not-here')).status,404);
  assert.equal((await request('/',{host:'attacker.invalid'})).status,421);
  await assert.rejects(request('/',{ca:null}),/self-signed|certificate/i);
});
test('rejects unauthorized or cross-origin pairing and data without delivery',async()=>{
  const payload=JSON.stringify([sample]);
  assert.equal((await request('/pair',{method:'POST',auth:null})).status,401);
  assert.equal((await request('/pair',{method:'POST',auth:'bad'})).status,401);
  assert.equal((await request('/pair',{method:'POST',origin:'https://attacker.invalid'})).status,403);
  assert.equal((await request('/pair',{method:'POST',origin:null})).status,403);
  assert.equal((await request('/pair',{method:'POST',contentType:'text/plain'})).status,403);
  assert.equal((await request('/gestures',{method:'POST',body:payload})).status,401);
  assert.equal(received.length,0);
});
test('concurrent sessions preserve independent identities, Pencil data and more than sixteen layers',async()=>{
  const a=await pair(),b=await pair();assert.notEqual(a.sourceId,b.sourceId);assert.notEqual(a.token,b.token);
  const gesture={...sample,sourceId:'untrusted',extra:'ignored'};
  const deliveries=await Promise.all([a,b].map(session=>request('/gestures',{method:'POST',body:JSON.stringify([gesture]),auth:session.token})));
  assert.deepEqual(deliveries.map(result=>result.status),[204,204]);
  assert.deepEqual(new Map(received.map(g=>[g.sourceId,g])),new Map([[a.sourceId,{sourceId:a.sourceId,...sample}],[b.sourceId,{sourceId:b.sourceId,...sample}]]));
  assert.equal((await request('/unpair',{method:'POST',auth:a.token})).status,204);
  assert.equal((await request('/gestures',{method:'POST',body:JSON.stringify([gesture]),auth:a.token})).status,401);
  assert.equal((await request('/gestures',{method:'POST',body:JSON.stringify([gesture]),auth:b.token})).status,204);
});
test('bounds data and rate without accepting malformed samples',async()=>{
  const {token:sessionToken}=await pair();
  const post=body=>request('/gestures',{method:'POST',body,auth:sessionToken});
  assert.equal((await post('{')).status,400);
  assert.equal((await post(JSON.stringify([{...sample,pressure:2}]))).status,400);
  assert.equal((await post(JSON.stringify(Array(33).fill(sample)))).status,400);
  assert.equal((await post('x'.repeat(17000))).status,413);
  let rejected=false;
  for(let i=0;i<20;i++)if((await post(JSON.stringify(Array(32).fill(sample)))).status===429){rejected=true;break;}
  assert.equal(rejected,true);
});
