import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function exerciseControls(windows) {
  const $ = id => document.getElementById(id);
  const check = (ok, message) => { if (!ok) throw Error(message); };
  async function until(predicate) {
    for (let i=0;i<500;i++) {
      if(predicate()) return;
      await new Promise(resolve=>setTimeout(resolve,10));
    }
    throw Error('Timed out: ' + $('app-status').textContent);
  }
  const idle = () => until(()=>!$('project-operation').open);
  const click = id => $(id).click();
  const set = (id,value) => {
    const input=$(id);input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));
  };
  const audio = new Uint8Array(await (await fetch('/sample.m4a')).arrayBuffer());
  let documentText, incoming, linked=null, lastSelection=1;
  if (windows) {
    Object.defineProperty(navigator,'userAgent',{value:'Windows project controls'});
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',audio)), b=>b.toString(16).padStart(2,'0')).join('');
    const events=['down','up'].map((phase,t)=>({phase,t,layerId:1,slide:'front',gesture:'pressure',amount:1-t,tugResponse:.4,point:[.5,.5]}));
    const macro={id:'take',name:'Pulse',duration:1,events,songStart:0,enabled:true,fit:true,cue:'recording-position',muted:[],amount:1,step:0,cap:2};
    localStorage.setItem('tss.gestures.v1.'+hash,JSON.stringify({macros:[macro],lyricGroups:[]}));
  }
  window.__TAURI__ = {event:{listen:async()=>()=>{}},core:{invoke:async (command,args)=>{
    switch(command) {
    case 'choose_audio': return [lastSelection,'/music/two-slide-cues.m4a'];
    case 'confirm_audio': case 'clear_project_audio': case 'remember_project_audio': return;
    case 'read_pending_audio': return lastSelection===9 ? new Uint8Array([1,2,3]) : audio.slice();
    case 'set_live_wheel': case 'forget_live_wheel_layer': case 'live_wheel_benchmark_config': return null;
    case 'live_wheel_stats': return {running:false};
    case 'read_timed_lyrics': return ['song.vtt','WEBVTT\n\n00:00.000 --> 00:00.500\nGo\n\n00:01.000 --> 00:01.500\nGo'];
    case 'save_project': documentText=args.document;return 'Headliner.tss';
    case 'open_project': return ['Headliner.tss',incoming,linked];
    default: throw Error('Unexpected command '+command);
    }
  }}};
  // This exercises the controls and real audio decoder with the GPU renderer idle.
  window.requestAnimationFrame=()=>0;
  await import('/app.js');
  await until(()=>$('edit-history').options.length>0);
  await window.tssImportAudio();
  if (windows) {
    set('wheel-motion','wheel-with-coral-beads');set('wheel-films','front');set('wheel-focus','71');
    set('wheel-slide','front');set('wheel-gesture','pressure');set('mouse-pressure','81');
    set('native-export-baseline','neutral');
  }
  click('add-layer'); set('speed','1.8'); click('save-project');await idle();
  const saved=JSON.parse(documentText);
  check(saved.layers.length===2 && saved.layers[0].speed===1.8 && saved.audio.sha256.length===64,'save controls');
  if (windows) check(saved.layers[1].visibleFilms==='front' && saved.focus===.71 && saved.layers[1].mousePressure.front===.81 &&
    saved.gestures.macros[0].name==='Pulse' && saved.gestures.lyricGroups[0].occurrences.length===2 && saved.exportBaseline==='neutral','native controls');
  click('undo-edit');await idle();check($('speed').value==='1','undo');
  click('redo-edit');await idle();check($('speed').value==='1.8','redo');
  incoming=JSON.stringify({...saved,version:2});click('open-project');await idle();
  check($('app-status').textContent.includes('Unsupported') && $('layer-count').textContent.startsWith('2'),'invalid open preserves session');
  incoming=documentText;click('add-layer');click('open-project');await until(()=>$('replace-project').open);
  $('replace-project').querySelector('[value=keep]').click();await idle();check($('layer-count').textContent.startsWith('3'),'cancel replacement');
  click('open-project');await until(()=>$('replace-project').open);$('replace-project').querySelector('[value=replace]').click();await idle();
  check(!$('relink-audio').hidden && $('play').disabled && $('layer-count').textContent.startsWith('2'),'offline project');
  lastSelection=9;await window.tssImportAudio(undefined,true);check($('play').disabled && $('app-status').textContent.includes('does not match'),'wrong song');
  lastSelection=2;await window.tssImportAudio(undefined,true);check(!$('play').disabled && $('relink-audio').hidden,'relink');
  linked=[2,'/other/two-slide-cues.m4a'];click('open-project');await idle();click('save-project');await idle();
  check(JSON.stringify(saved)===documentText,'save-open-save identity');
}
const chrome=process.env.CHROME_BIN;
for (const windows of [false,true]) test((windows?'Native wheel':'Procedural')+' editor save, undo, redo, cancel, offline open, relink and reopen through its controls',
  {skip:!chrome && 'Set CHROME_BIN to a Chrome executable', timeout:30000}, async()=>{
    const web=new URL('../../WebAssets/',import.meta.url);
    const media=new URL('../slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a',import.meta.url);
    const scratch=await mkdtemp(join(tmpdir(),'tss-project-ui-'));
    let report;
    const done=new Promise(resolve=>{report=resolve;});
    const server=createServer(async(req,res)=>{
      try {
        if(req.url==='/report') {let body='';for await(const part of req)body+=part;res.end();report(body);return;}
        if(req.url==='/') {
          const html=(await readFile(new URL('index.html',web),'utf8')).replace('<script type="module" src="app.js"></script>',
            '<script type="module">('+exerciseControls.toString()+')('+windows+').then(()=>fetch("/report",{method:"POST",body:"pass"})).catch(e=>fetch("/report",{method:"POST",body:e.stack}));</script>');
          res.writeHead(200,{'Content-Type':'text/html'});res.end(html);return;
        }
        if(req.url==='/sample.m4a'){res.writeHead(200,{'Content-Type':'audio/mp4'});res.end(await readFile(media));return;}
        const allowed=/^\/(?:[a-z-]+\.js|styles\.css|assets\/(?:mathmos-wheels\.json|tss-project-logo\.jpeg))$/;
        if(!allowed.test(req.url)){res.writeHead(404);res.end();return;}
        const type=req.url.endsWith('.js')?'text/javascript':req.url.endsWith('.css')?'text/css':req.url.endsWith('.json')?'application/json':'image/jpeg';
        res.writeHead(200,{'Content-Type':type});res.end(await readFile(new URL(req.url.slice(1),web)));
      } catch(e) {res.destroy(e);report(String(e));}
    });
    await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
    const proc=spawn(chrome,['--headless','--disable-gpu','--disable-webgl','--disable-software-rasterizer',
      '--disable-background-networking','--no-first-run','--user-data-dir='+scratch,'http://127.0.0.1:'+server.address().port],{stdio:'ignore'});
    const exited=new Promise(resolve=>{proc.once('exit',resolve);proc.once('error',e=>{report(String(e));resolve();});});
    let timer;
    try {
      const timeout=new Promise(resolve=>{timer=setTimeout(()=>resolve('Browser timed out'),24000);});
      assert.equal(await Promise.race([done,timeout,exited.then(()=> 'Browser exited')]),'pass');
    } finally {
      clearTimeout(timer);proc.kill();await exited;server.closeAllConnections();server.close();
      await rm(scratch,{recursive:true,force:true,maxRetries:5,retryDelay:50});
    }
  });
