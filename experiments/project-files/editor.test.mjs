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
  const advisorRequests=[],reviewedLyrics=[];
  let finishInference;
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
    case 'transcribe_song':return [
      {start:0,end:.25,text:'Before the selection'},
      {start:.3,end:.75,text:'Machine draft'},
      {start:1.25,end:1.75,text:'After the selection'},
    ];
    case 'save_reviewed_lyrics':reviewedLyrics.push(args.text);return true;
    case 'suggest_visual_layer':
      advisorRequests.push(args);
      return new Promise(resolve=>{finishInference=()=>resolve({visual:'video',signal:'onset',fit:'repeat'});});
    case 'resolve_visual_media': return args.name==='Plate.gif'?{id:42,name:args.name,sha256:args.sha256,width:400,height:240,durationUs:800000,still:false}:null;
    case 'open_project': return ['Headliner.tss',incoming,linked];
    default: throw Error('Unexpected command '+command);
    }
  }}};
  // This exercises the controls and real audio decoder with the GPU renderer idle.
  window.requestAnimationFrame=()=>0;
  await import('/app.js');
  await until(()=>$('edit-history').options.length>0);
  await window.tssImportAudio();
  $('advice-panel').open=true;
  set('advice-direction','Violet at the downbeat');click('advice-suggest');
  check(!advisorRequests.length && $('advice-status').textContent.includes('Select a timeline region'),'advisor requires an operator selection');
  $('lyrics-deck').open=true;click('lyrics-run');await until(()=>!$('lyrics-review').hidden);
  set('lyrics-draft',$('lyrics-draft').value.replace('Machine draft','Reviewed lyric'));
  click('lyrics-save');await until(()=>reviewedLyrics.length===1);
  check(reviewedLyrics[0].includes('Reviewed lyric'),'approved lyrics');
  set('range-start','.25');set('range-end','1.25');click('set-range');
  check(!$('advice-lyrics').checked,'lyric excerpts default off');
  click('advice-suggest');await until(()=>advisorRequests.length===1);
  check(advisorRequests[0].direction==='Violet at the downbeat','no lyric or transcript text without consent');
  set('range-end','1.1');click('set-range');finishInference();
  await until(()=>!$('advice-suggest').disabled);
  check($('advice-review').hidden && $('advice-status').textContent.includes('selection changed') &&
    $('layer-count').textContent.startsWith('1'),'pending inference on a stale range cannot alter the arrangement');
  set('range-end','1.25');click('set-range');click('advice-lyrics');click('advice-suggest');
  await until(()=>advisorRequests.length===2);
  check(advisorRequests[1].direction==='Violet at the downbeat\nLyric excerpt: Reviewed lyric' && advisorRequests[1].port===18081,
    'only opted-in reviewed cue text overlapping this range reaches inference');
  finishInference();await until(()=>!$('advice-review').hidden);
  check($('advice-apply').disabled && $('advice-visual').value==='video' && $('advice-fit').value==='repeat' &&
    $('advice-range').textContent==='0.25–1.25 s','review precedes any track change');
  set('advice-visual','oil');set('advice-signal','time');set('advice-fit','elastic');set('advice-cue','Reviewed blue oil');
  click('advice-apply');await until(()=>$('advice-status').textContent.includes('Added the reviewed oil layer'));
  check($('layer-count').textContent.startsWith('2') && $('selected-name').textContent==='Reviewed blue oil','edited suggestion adds a real selected track');
  click('save-project');await idle();
  const applied=JSON.parse(documentText).layers[0];
  check(applied.name==='Reviewed blue oil' && applied.envelope.length===1 && applied.envelope[0].start===.25 &&
    applied.envelope[0].end===1.25 && applied.envelope[0].rule.mode==='stretch','granted range and edited fit persisted');
  click('undo-edit');await idle();
  check($('layer-count').textContent.startsWith('1') && $('selected-name').textContent!=='Reviewed blue oil','advisor track is undoable');
  click('deselect-time');click('advice-lyrics');
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
  set('range-start','0.25');set('range-end','1.25');click('set-range');
  set('wand-mode','stretch');set('wand-in','.2');set('wand-out','.35');set('wand-level','.6');click('apply-wand');
  click('save-region-project');await idle();
  const region=JSON.parse(documentText);
  check(region.timeline.range.start===.25 && region.timeline.range.end===1.25 &&
    region.layers[0].envelope.length===1 && region.layers[0].envelope[0].fadeOut===.35,'saved region preserves independent fades');
  incoming=documentText;
  set('template-name','Verse fold');click('save-song-track');await idle();click('save-user-template');await idle();
  check([...$('track-source').options].some(o=>o.text==='Song · Verse fold') &&
    [...$('track-source').options].some(o=>o.text==='Template · Verse fold'),'song and reusable libraries');
  click('add-layer');await idle();check($('layer-count').textContent.startsWith('3'),'add template track');
  click('clear-region');await idle();click('save-project');await idle();
  check(JSON.parse(documentText).layers[0].envelope.length===0,'regional clear');
  click('undo-edit');await idle();
  click('merge-region-project');await idle();click('save-project');await idle();
  const merged=JSON.parse(documentText);
  check(merged.layers.length===5 && new Set(merged.layers.map(l=>l.id)).size===5 && merged.layers[0].envelope[0].start===.25,'regional merge');
  if(windows)check(merged.gestures.macros.length===2 && merged.gestures.macros[1].events[0].layerId===merged.layers[1].id,'regional gesture remap');
  const media={name:'Plate.gif',sha256:'a'.repeat(64),width:200,height:120,durationUs:800000,still:false,
    offsetUs:0,rate:1,loop:true,mode:'color',keyEnabled:false,keyColor:'#00FF00',keyThreshold:.2,keySoftness:.1,
    gradientA:'#000000',gradientB:'#FFFFFF',gradientRate:0,blur:0,distortion:0,bounce:0,maskA:null,maskB:null,
    maskOp:'union',shape:'none',assetId:999};
  merged.layers[0].media=media;
  merged.layers[1].media={...media,name:'Offline.gif',sha256:'b'.repeat(64)};
  incoming=JSON.stringify(merged);click('open-project');await idle();
  check($('app-status').textContent.includes('Offline.gif'),'offline visual source named');
  click('save-project');await idle();const restored=JSON.parse(documentText);
  check(restored.layers[0].media.width===400 && restored.layers[1].media.width===200 &&
    !restored.layers.some(l=>l.media?.assetId!==undefined),'portable media link, offline metadata and refreshed grant');
}
const chrome=process.env.CHROME_BIN;
for (const windows of [false,true]) test((windows?'Native wheel':'Procedural')+' editor project and region controls, reviewed advisor cues, lyric consent and undo',
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
        const allowed=/^\/(?:[a-z-]+\.js|styles\.css|assets\/(?:mathmos-wheels\.json|visual-advice\.json|tss-project-logo\.jpeg))$/;
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
