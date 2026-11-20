import https from 'node:https';
import { randomBytes, timingSafeEqual, createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const here = new URL('.', import.meta.url);
const files = new Map([
  ['/', [new URL('index.html', here), 'text/html; charset=utf-8']],
  ['/client.mjs', [new URL('client.mjs', here), 'text/javascript; charset=utf-8']],
  ['/style.css', [new URL('style.css', here), 'text/css; charset=utf-8']],
  ['/gesture-input.mjs', [new URL('../../performer/surface/gesture-input.mjs', here), 'text/javascript; charset=utf-8']],
]);
const MAX_BODY = 16384, MAX_BATCH = 32;

function validRecord(g) {
  const bounded = (n, min, max) => typeof n === 'number' && Number.isFinite(n) && n >= min && n <= max;
  const optional = (n, min, max) => n === null || bounded(n, min, max);
  return g && typeof g === 'object' && !Array.isArray(g) &&
    bounded(g.time, 0, 1e10) && Number.isSafeInteger(g.pointerId) && g.pointerId >= 0 &&
    ['mouse','pen','touch'].includes(g.pointerType) && ['down','move','up','cancel'].includes(g.phase) &&
    bounded(g.x,0,1) && bounded(g.y,0,1) && optional(g.pressure,0,1) &&
    [null,'pointer','webkitForce'].includes(g.pressureSource) && optional(g.rawForce,0,1e6) &&
    optional(g.tiltX,-90,90) && optional(g.tiltY,-90,90) &&
    Array.isArray(g.layerIds) && g.layerIds.every(v => (typeof v === 'string' && v.length <= 64) || (Number.isSafeInteger(v) && v >= 0));
}
function cleanRecord(g, sourceId) {
  const {time,pointerId,pointerType,phase,x,y,pressure,pressureSource,rawForce,tiltX,tiltY,layerIds} = g;
  // A server-assigned connection source avoids trusting an identity supplied by the controller.
  return {sourceId,time,pointerId,pointerType,phase,x,y,pressure,pressureSource,rawForce,tiltX,tiltY,layerIds};
}
function respond(res,status,body='') { res.writeHead(status,{'Cache-Control':'no-store','Content-Type':'text/plain; charset=utf-8','X-Content-Type-Options':'nosniff'}); res.end(body); }

export async function startController({key,cert,bindHost,port,publicHost,token,onGestures,maxSessions=1024,ratePerSecond=5000}) {
  if (!key || !cert || !bindHost || !publicHost || !Number.isInteger(port) || port < 0 || port > 65535 ||
      !/^[A-Za-z0-9.-]+$/.test(publicHost) || !/^[A-Za-z0-9_-]{43}$/.test(token) || typeof onGestures !== 'function' || !Number.isSafeInteger(maxSessions) || maxSessions < 1 || !Number.isSafeInteger(ratePerSecond) || ratePerSecond < 1) throw new TypeError('Invalid server configuration');
  const payloads = new Map();
  for (const [path,[url,type]] of files) payloads.set(path,[await readFile(url),type]);
  const secret = Buffer.from(token);
  const sessions = new Map();
  const digest = value => createHash('sha256').update(value).digest('hex');
  let allowedOrigin, bucketStart=Date.now(), used=0;
  const server=https.createServer({key,cert},(req,res)=>{
    const rawHost=req.headers.host;
    if (rawHost !== new URL(allowedOrigin).host) return respond(res,421);
    if (req.method === 'GET' && payloads.has(req.url)) {
      const [data,type]=payloads.get(req.url);
      res.writeHead(200,{'Content-Type':type,'Cache-Control':'no-store','X-Content-Type-Options':'nosniff',
        'Content-Security-Policy':"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'",'X-Frame-Options':'DENY'});
      return res.end(data);
    }
    if (req.method !== 'POST' || !['/pair','/gestures','/unpair'].includes(req.url)) return respond(res,404);
    if (req.headers.origin !== allowedOrigin || req.headers['content-type'] !== 'application/json') return respond(res,403);
    const auth=req.headers.authorization || '';
    const supplied=Buffer.from(auth.startsWith('Bearer ') ? auth.slice(7) : '');
    if (req.url === '/pair') {
      if (supplied.length !== secret.length || !timingSafeEqual(supplied,secret)) return respond(res,401);
      if (sessions.size >= maxSessions) return respond(res,503);
      const sessionToken=randomBytes(32).toString('base64url'), sourceId=randomUUID();
      sessions.set(digest(sessionToken),sourceId);
      res.writeHead(200,{'Cache-Control':'no-store','Content-Type':'application/json','X-Content-Type-Options':'nosniff'});
      return res.end(JSON.stringify({token:sessionToken,sourceId}));
    }
    if (supplied.length !== secret.length) return respond(res,401);
    const sessionKey=digest(supplied), sourceId=sessions.get(sessionKey);
    if (!sourceId) return respond(res,401);
    if(req.url === '/unpair'){sessions.delete(sessionKey);return respond(res,204);}
    if (req.headers['content-length'] && Number(req.headers['content-length']) > MAX_BODY) { req.resume(); return respond(res,413); }
    let count=0, chunks=[], rejected=false;
    req.on('data',chunk=>{
      count+=chunk.length;
      if (count > MAX_BODY) { if (!rejected) {rejected=true;respond(res,413);} chunks=[]; }
      else if (!rejected) chunks.push(chunk);
    });
    req.on('end',()=>{
      if(rejected) return;
      let input;
      try {input=JSON.parse(Buffer.concat(chunks).toString('utf8'));} catch {return respond(res,400);}
      if (!Array.isArray(input) || input.length<1 || input.length>MAX_BATCH || !input.every(validRecord)) return respond(res,400);
      const now=Date.now(); if(now-bucketStart>=1000){bucketStart=now;used=0;}
      if(used+input.length>ratePerSecond) return respond(res,429);
      used+=input.length;
      try {onGestures(input.map(g=>cleanRecord(g,sourceId)));} catch {return respond(res,500);}
      respond(res,204);
    });
  });
  server.requestTimeout=10000; server.headersTimeout=10000; server.maxHeadersCount=32;
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,bindHost,()=>{server.off('error',reject);resolve();});});
  allowedOrigin=new URL(`https://${publicHost}:${server.address().port}`).origin;
  return {server,origin:allowedOrigin,close:()=>new Promise((resolve,reject)=>server.close(error=>error?reject(error):resolve()))};
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  // Explicit CLI options: listen on a chosen interface and use an already trusted TLS certificate.
  const args=process.argv.slice(2);
  if(args.length!==5){console.error('Usage: node server.mjs BIND_HOST PUBLIC_HOST PORT TLS_KEY TLS_CERT');process.exit(2);}
  const [bindHost,publicHost,portText,keyPath,certPath]=args;
  if (!process.stdout.isTTY) {console.error('Run interactively in a terminal; the pairing secret is displayed once.');process.exit(2);}
  const token=randomBytes(32).toString('base64url');
  const app=await startController({key:await readFile(keyPath),cert:await readFile(certPath),bindHost,publicHost,port:Number(portText),token,maxSessions:Number(process.env.TSS_MAX_SESSIONS || 1024),ratePerSecond:Number(process.env.TSS_GESTURE_RATE || 5000),
    onGestures:(batch)=>{process.stdout.write(`Received ${batch.length} gesture sample(s).\n`);}});
  process.stdout.write(`Open ${app.origin}/ on the paired device. Enter this one-session secret: ${token}\n`);
  for(const signal of ['SIGINT','SIGTERM']) process.once(signal,async()=>{await app.close();process.exit(0);});
}
