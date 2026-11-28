import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function exerciseControls(platform) {
  if (platform === 'Macintosh pressure meter') {
    const {TrackpadMeter} = await import('/trackpad-meter.js');
    const root = document.getElementById('trackpad-meter');
    const slider = document.getElementById('trackpad-sensitivity');
    const force = document.getElementById('trackpad-force');
    const load = document.getElementById('trackpad-load');
    const check = (condition, message) => { if (!condition) throw Error(message); };
    let event, changes = 0;
    const listen = async (name, listener) => {
      check(name === 'tss-trackpad-pressure', 'subscribe to measured force');
      event = listener;
    };
    const meter = new TrackpadMeter(root, () => changes++, listen);
    await meter.registration;
    const front = {layerId:3, slide:'rear', name:'Rose oil'};
    meter.setTarget(front);
    check(meter.sensitivity === 1 && !root.hidden && root.textContent.includes('Rose oil'), 'visible target and linear default');
    event({payload:{...front, raw:.25, applied:.25}});
    check(force.value === 25 && load.value === 25, 'immediate quarter force');
    slider.value = 1;
    slider.dispatchEvent(new Event('input'));
    slider.dispatchEvent(new Event('change'));
    check(changes === 1 && meter.sensitivity === 2 && slider.getAttribute('aria-valuetext') === '2×', 'sensitivity control');
    event({payload:{...front, raw:.25, applied:.5}});
    check(force.value === 25 && load.value === 50, 'native response with measured force unchanged');
    meter.setTarget({...front, layerId:7});
    event({payload:{...front, raw:1, applied:1}});
    check(force.value === 0 && load.value === 0, 'ignore stale force after selection changes');
    event({payload:{...front, layerId:7, raw:1, applied:1}});
    event({payload:{...front, layerId:7, raw:0, applied:0}});
    check(force.value === 0 && load.value === 0, 'release clears meters');
    meter.setTarget(null);
    check(root.hidden, 'hide outside pressure mode');
    const reopened = new TrackpadMeter(root.cloneNode(true), () => {}, listen);
    check(reopened.sensitivity === 2, 'operator sensitivity survives relaunch');
    const depth = root.querySelector('#stereo-depth');
    const depthControls = root.querySelector('#stereo-depth-controls');
    check(meter.stereoDepth === .2 && depth.value === '20' && depthControls.hidden, 'default stereo separation');
    meter.setStereoActive(true);
    check(!root.hidden && !depthControls.hidden && root.querySelector('.force-inputs').hidden, 'depth independent of pressure target');
    depth.value = '100';
    depth.dispatchEvent(new Event('input'));
    check(meter.stereoDepth === 1 && changes === 2, 'full separation updates config');
    depth.value = '0';
    depth.dispatchEvent(new Event('input'));
    depth.dispatchEvent(new Event('change'));
    check(meter.stereoDepth === 0 && depth.getAttribute('aria-valuetext') === '0%', 'flat stereo');
    const restored = new TrackpadMeter(root.cloneNode(true), () => {}, listen);
    check(restored.stereoDepth === 0 && restored.sensitivity === 2, 'flat output and pressure sensitivity persist independently');
    meter.setStereoActive(false);
    check(root.hidden, 'hide stereo settings after stop');
    return;
  }
  const coral = platform === 'Windows coral project';
  const oils = platform === 'Windows film oils';
  const windows = platform === 'Windows';
  const solver = platform === 'Windows live solver';
  const benchmark = platform === 'Windows recorded benchmark';
  const stereo = platform === 'Macintosh stereo session';
  const missing = solver && location.hash === '#missing-helper';
  const helperAvailable = platform !== 'Linux' && !missing;
  let resolveHelper;
  Object.defineProperty(navigator,'userAgent',{value:platform+' project controls'});
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
  const audio = coral || oils || missing || benchmark || stereo ? null : new Uint8Array(await (await fetch('/sample.m4a')).arrayBuffer());
  let documentText, incoming, linked=null, lastSelection=1, catalogFiles=[];
  const advisorRequests=[],reviewedLyrics=[],previewRequests=[],movieRequests=[];
  const listeners=new Map(), stereoRequests=[], forgotten=[];
  let stereoFails=false, stereoFrames=73, stereoFps=22.37, stereoPolls=0, projectorVisible;
  let scaleQuery;
  if (stereo) {
    const browserMatchMedia = window.matchMedia.bind(window);
    window.matchMedia = media => {
      const query = browserMatchMedia(media);
      if (media.startsWith('(resolution:')) scaleQuery = query;
      return query;
    };
  }
  let finishInference;
  let mediaChecks=0,mediaInstalls=0,mediaReady=false;
  if (windows) {
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',audio)), b=>b.toString(16).padStart(2,'0')).join('');
    const events=['down','up'].map((phase,t)=>({phase,t,layerId:1,slide:'front',gesture:'pressure',amount:1-t,tugResponse:.4,point:[.5,.5]}));
    const macro={id:'take',name:'Pulse',duration:1,events,songStart:0,enabled:true,fit:true,cue:'recording-position',muted:[],amount:1,step:0,cap:2};
    localStorage.setItem('tss.gestures.v1.'+hash,JSON.stringify({macros:[macro],lyricGroups:[]}));
  }
  window.__TAURI__ = {event:{listen:async(name,cb)=>{listeners.set(name,cb);return ()=>listeners.delete(name);}},core:{invoke:async (command,args)=>{
    switch(command) {
    case 'available_legend_packs': return catalogFiles;
    case 'choose_visual_media': throw Error('Could not find ffmpeg. Open FFmpeg media tools in the editor to install or recheck.');
    case 'check_media_tools':
      mediaChecks++;
      return {ready:mediaReady,managerAvailable:true,installCommand:'brew install ffmpeg',managerHelp:'',
        tools:[{name:'ffmpeg',version:mediaReady?'9.0.1':null,path:mediaReady?'/usr/local/bin/ffmpeg':null,issues:mediaReady?[]:['Could not find ffmpeg.']},
          {name:'ffprobe',version:'9.0.1',path:'/usr/local/bin/ffprobe',issues:[]}]};
    case 'install_media_tools':
      mediaInstalls++;
      throw Error('Package download failed.');
    case 'live_simulation_available': return missing ? new Promise(resolve=>{resolveHelper=resolve;}) : helperAvailable;
    case 'choose_audio': return [lastSelection,'/music/two-slide-cues.m4a'];
    case 'choose_export_path': return '/movies/two-slide-cues.mp4';
    case 'export_native_wheel': movieRequests.push(args);return;
    case 'confirm_audio': case 'clear_project_audio': case 'remember_project_audio': return;
    case 'set_visual_audio_features':
      check(args.fps>0 && args.frames.every(f=>f.length===4 && f.every(x=>Number.isFinite(x) && x>=0 && x<=1)), 'native audio modulation envelope');
      return;
    case 'read_pending_audio': return lastSelection===9 ? new Uint8Array([1,2,3]) : audio.slice();
    case 'set_live_wheel': previewRequests.push(args.config);return null;
    case 'live_wheel_benchmark_config': return benchmark ? {wheel:'violet-blue',motion:'wheel-flow',slide:'rear',gesture:'tug',renderWidth:640,wheelStack:true} : null;
    case 'stereo_startup_requested': return stereo;
    case 'set_stereo_output':
      stereoRequests.push(args.enabled);
      if (args.enabled && stereoFails) throw Error('No external display available');
      return args.enabled ? 'SAMSUNG' : '';
    case 'forget_live_wheel_layer': forgotten.push(args.layerId);return null;
    case 'live_wheel_stats':
      if (!stereo || !stereoRequests.at(-1)) return {running:false};
      stereoPolls++;
      return {
        running:true,layerId:previewRequests.at(-1)?.layerId,adapter:'Metal',fps:stereoFps,p95Ms:46,
        width:640,height:360,frameLatency:2,projectorRunning:true,projectorPresents:stereoFrames,projectorVisible,
        trackpadPressure:.42,trackpadPressurePeak:.78,trackpadPressureEvents:17,
      };
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
    case 'open_project': return [coral || oils ? 'coral-wheel.tss' : 'Headliner.tss',incoming,linked];
    default: throw Error('Unexpected command '+command);
    }
  }}};
  // This exercises the controls and real audio decoder with the GPU renderer idle.
  window.requestAnimationFrame=()=>0;
  await import('/app.js');
  if (missing) {
    await until(()=>!!resolveHelper);
    click('add-layer');
    check(!previewRequests.some(Boolean),'an unresolved solver probe cannot present recorded motion');
    resolveHelper(false);
  }
  await until(()=>$('edit-history').options.length>0);
  if (stereo) {
    const button=$('projector');
    await until(()=>stereoRequests.at(-1)===true && !button.disabled);
    const rig=previewRequests.at(-1);
    const original=previewRequests.find(c=>c?.simulation && !c.hardwarePressureOnly);
    const trackCount=$('layer-count').textContent;
    check(JSON.stringify(rig.stack)===JSON.stringify(original.stack),'stereo uses every authored dye and original solver seed in project order');
    check(rig.layerId===rig.stack.at(-1).layerId && rig.hardwarePressureOnly &&
      rig.gesture==='pressure' && rig.slide==='rear' && rig.focus===original.focus && rig.renderWidth===960,'startup binds pressure to the front project film');
    check(rig.stereoDepth===.2 && rig.pressureSensitivity===1,'stereo depth and force begin at their defaults');
    const latest=()=>previewRequests.at(-1);
    const identity=stack=>stack.map(t=>[t.layerId,t.simulation.seed,t.simulation.depth]);
    const originalIdentity=JSON.stringify(identity(original.stack));
    check($('wheel-width-label').textContent === 'Resolution per eye' && $('wheel-width').value === '960',
      'quality selector shows the actual stereo resolution');
    set('wheel-width','640');
    await until(()=>latest()?.renderWidth===640);
    set('wheel-width','960');
    await until(()=>latest()?.renderWidth===960);
    const presets=await (await fetch('/assets/slide-presets.json')).json();
    for (const preset of presets.wheels) {
      set('slide-preset',preset.id);
      await until(()=>latest()?.stack?.every(t=>t.wheel===preset.id));
      check(latest().stack.every((t,i)=>t.swatches[1]===parseInt(preset.dyes[i].slice(1),16)),'project dye on each slide in stereo: '+preset.id);
      check(JSON.stringify(identity(latest().stack))===originalIdentity,'palette edits preserve seeds, depth and track identity');
    }
    const rows=$('layer-list').querySelectorAll('.layer-row');
    rows[rows.length-1].click();
    await until(()=>latest()?.layerId===original.layerId);
    const picker=$('pigment-colors').querySelector('input');
    picker.value='#25bc71'; picker.dispatchEvent(new Event('input')); picker.dispatchEvent(new Event('change',{bubbles:true}));
    await until(()=>latest()?.swatches?.[1]===0x25bc71);
    check(latest().stack.find(t=>t.layerId===original.layerId).swatches[1]===0x25bc71,'active shader and composite use the same edited dye');
    set('hue','90');
    await until(()=>latest().swatches[1]!==0x25bc71);
    set('hue','0');
    set('speed','0'); set('scale','.4'); set('opacity','.65'); set('film-depth','43'); set('wheel-focus','72');
    await until(()=>latest()?.stack?.some(t=>t.layerId===original.layerId && t.simulation.rate===0 && t.detailScale===.4 &&
      t.opacity===.65 && t.simulation.depth===.43 && t.swatches[1]===0x25bc71) && latest().focus===.72);
    check(latest().stack.find(t=>t.layerId!==original.layerId).simulation.rate===1,'flow pause targets only the chosen layer in stereo');
    check($('scale-output').textContent.includes('2.50× zoom'),'field scale reports actual magnification');
    set('speed','1'); set('scale','1'); set('opacity','1'); click('wheel-focus-reset');
    await until(()=>latest()?.simulation?.rate===1 && latest().focus===original.focus);
    const editedDepths = JSON.stringify(identity(latest().stack));
    set('trackpad-sensitivity','-1');
    await until(()=>latest()?.pressureSensitivity===.5);
    set('stereo-depth','65');
    await until(()=>latest()?.stereoDepth===.65);
    check(JSON.stringify(identity(latest().stack))===editedDepths,'pressure sensitivity and stereo depth preserve edited depth and source identities');
    click('save-project'); await idle();
    const saved=JSON.parse(documentText);
    check(saved.layers.length===original.stack.length && saved.selected===original.layerId &&
      saved.layers.find(l=>l.id===original.layerId).colors[0]==='#25bc71','save during stereo preserves project layers, selection and edited color');
    const edits=$('edit-history').options.length;
    const scale=devicePixelRatio, css=$('preview').getBoundingClientRect();
    const pixelRect = (ratio) => {
      const px = x => Math.round(x*ratio);
      return {x:px(css.left),y:px(css.top),width:px(css.right)-px(css.left),height:px(css.bottom)-px(css.top)};
    };
    Object.defineProperty(window,'devicePixelRatio',{value:scale*2,configurable:true});
    const oldQuery=scaleQuery;
    oldQuery.dispatchEvent(new Event('change'));
    await until(()=>JSON.stringify(previewRequests.at(-1).rect)===JSON.stringify(pixelRect(scale*2)));
    check(oldQuery!==scaleQuery,'resolution change rearms for the new monitor scale');
    Object.defineProperty(window,'devicePixelRatio',{value:scale,configurable:true});
    await until(()=>JSON.stringify(previewRequests.at(-1).rect)===JSON.stringify(pixelRect(scale)));
    const hud = () => $('stereo-pressure-state').textContent;
    await until(()=>stereoPolls >= 2 && hud().includes('TV 22.4 FPS'));
    check(hud().includes('73 stereo frames · peak 78% · 17 pressure events'),
      'duplicate native samples retain presenter FPS, frame count and force telemetry');
    check(!hud().includes('screen'), 'unknown TV visibility makes no visibility claim');
    for (const fps of [0, -1, Infinity, NaN]) {
      stereoFps = fps;
      const before = stereoPolls;
      await until(()=>stereoPolls > before && hud().includes('TV measuring'));
    }
    stereoFrames = 1; stereoFps = 30; projectorVisible = true;
    await until(()=>hud().includes('TV measuring · 1 stereo frame') && hud().includes('TV on screen'));
    stereoFrames = 2; projectorVisible = false;
    await until(()=>hud().includes('TV 30.0 FPS · 2 stereo frames') && hud().includes('TV window off screen'));
    click('projector');await until(()=>!button.disabled && stereoRequests.at(-1)===false);
    check(stereoRequests.at(-1)===false && previewRequests.at(-1).layerId===original.layerId &&
      previewRequests.at(-1).simulation.seed===original.simulation.seed && $('stereo-pressure-state').hidden,'stop restores mono simulation');
    check($('layer-count').textContent===trackCount && $('edit-history').options.length===edits &&
      latest().swatches[1]===0x25bc71 && latest().renderWidth===original.renderWidth && !latest().hardwarePressureOnly,
      'stopping stereo preserves edits and restores mono preview width and gesture');
    stereoFails=true;click('projector');
    await until(()=>$('app-status').textContent.includes('No external display') && !button.disabled);
    check(previewRequests.at(-1).layerId===original.layerId && forgotten.length===0,'failed start retains project solvers');
    stereoFails=false;click('projector');await until(()=>button.getAttribute('aria-pressed')==='true' && !button.disabled);
    await listeners.get('tss-stereo-closed')({payload:'Display disconnected'});
    check(button.getAttribute('aria-pressed')==='false' && forgotten.length===0 && previewRequests.at(-1).layerId===original.layerId,'disconnect restores project input');
    const recorded=saved.layers.find(l=>l.id===saved.selected);
    delete recorded.simulationSeed; delete recorded.filmDepth; delete recorded.presetGroup; delete recorded.dyeIndex;
    recorded.colors=['#ffffff','#aa88aa','#335533','#aaff00'];
    incoming=JSON.stringify(saved); click('open-project');
    await until(()=>$('replace-project').open || !$('project-operation').open);
    if ($('replace-project').open) $('replace-project').querySelector('[value=replace]').click();
    await idle(); click('projector');
    await until(()=>button.getAttribute('aria-pressed')==='true' && !button.disabled);
    check(latest().layerId!==recorded.id && latest().stack.some(t=>t.layerId===recorded.id && !t.simulation) &&
      $('wheel-slide').disabled && $('live-wheel').checked, 'mixed project keeps recorded geometry and routes pressure to the live film');
    click('live-wheel');
    await until(()=>!latest().stack?.some(t=>t.layerId===recorded.id));
    check(!$('live-wheel').checked && latest().hardwarePressureOnly, 'legacy preview checkbox reflects the selected track');
    return;
  }
  if (benchmark) {
    await until(()=>previewRequests.at(-1)?.stack?.length===2);
    const stack=previewRequests.at(-1).stack;
    check(stack.some(l=>l.wheel==='violet-blue') && stack.some(l=>l.wheel==='yellow-red') &&
      stack.every(l=>!l.simulation && l.motion==='wheel-flow'),'explicit benchmark preserves the historical two-slide workload');
    return;
  }
  if (solver) {
    const latest = () => previewRequests.at(-1);
    const save = async () => { click('save-project'); await idle(); return JSON.parse(documentText); };
    const reopen = async text => {
      incoming=text;
      click('open-project');
      await until(()=>$('replace-project').open || !$('project-operation').open);
      if ($('replace-project').open) $('replace-project').querySelector('[value=replace]').click();
      await idle();
    };
    const option=$('wheel-motion').querySelector('[value=simulated]');
    if (!helperAvailable) {
      check($('legend-title')?.textContent==='Studio signed-off rig','installed pack survives reopening the editor');
      check(!$('solver-unavailable').hidden,'fresh startup reports the unavailable solver');
      click('add-layer');
      const newProject=await save();
      check(newProject.layers.length>=2 && newProject.layers.every(l=>Number.isInteger(l.simulationSeed)),
        'new oil tracks retain their simulation seeds even when the solver is unavailable');
      check(!previewRequests.some(Boolean),'fresh oil tracks cannot fall back to recorded native fields');
      const authored=JSON.parse(sessionStorage.getItem('seeded-project'));
      const start=previewRequests.length;
      await reopen(JSON.stringify(authored));
      check(!$('solver-unavailable').hidden,'operator sees the missing helper when opening a seeded project');
      check(!previewRequests.slice(start).some(Boolean),'reopened oil cannot fall back to recorded native fields');
      const notices=['app-status','preview-status','native-frame-rate','native-instruction'].map(id=>$(id).textContent).join(' ');
      const preserved=await save();
      check(preserved.layers.every((l,i)=>l.simulationSeed===authored.layers[i].simulationSeed),
        'opening without the solver must preserve the authored tracks');
      check(/(?:solver|simulation|Basilisk|helper).*(?:unavailable|missing|required|install)|(?:unavailable|missing|required|install).*(?:solver|simulation|Basilisk|helper)/i.test(notices),
        'opening seeded tracks explicitly identifies the unavailable helper: '+notices);
      return;
    }

    await until(()=>latest()?.simulation!=null);
    const initial=latest(), firstSeed=initial.simulation.seed;
    const recipes=(await (await fetch('/assets/slide-presets.json')).json()).wheels;
    const baseCount=recipes.find(r=>r.id===initial.wheel).dyes.length;
    const baseSeeds=initial.stack.map(l=>l.simulation.seed);
    check(baseSeeds.length===baseCount && new Set(baseSeeds).size===baseCount,'one independent initial seed per physical dye');
    check($('pigment-colors').querySelectorAll('input').length===1 && $('wheel-slide').disabled,
      'selected physical layer exposes one dye and one gesture target');
    set('film-depth','23');await until(()=>latest()?.simulation?.depth===.23);
    const depths=latest().stack.map(l=>l.simulation.depth);
    check(Number.isInteger(firstSeed) && !option.disabled && $('wheel-motion').value==='simulated' &&
      !$('speed').disabled && !$('scale').disabled,'ordinary desktop startup uses a live physical track');
    check(previewRequests.filter(Boolean).every(c=>c.simulation),'all ordinary startup requests use a live solve');
    check($('wheel-motion').disabled || $('wheel-motion').hidden ||
      [...$('wheel-motion').options].every(o=>o.value==='simulated'||o.disabled||o.hidden),
      'recorded motion is not offered when authoring an oil track');
    const presets=[...$('slide-preset').options].map(o=>o.value).filter(Boolean);
    check(presets.length===7,'seven calibrated slide palettes');
    for (const name of presets) {
      set('slide-preset',name);
      await until(()=>latest()?.wheel===name);
      const dyes=recipes.find(r=>r.id===name).dyes.map(s=>parseInt(s.slice(1),16));
      check(latest().stack.length===dyes.length && latest().stack.every((l,i)=>l.simulation.seed===baseSeeds[i] &&
        l.simulation.depth===depths[i] && l.visibleFilms==='rear' && l.swatches.join() === [0xffffff,dyes[i],dyes[i],0xffffff].join()),
        'choosing '+name+' assigns one dye per cell and preserves seeds and optics');
    }
    for (const [i,name] of ['orchid','lagoon','ember','twilight','sunrise','meadow'].entries()) {
      $('palette-grid').querySelectorAll('button')[i].click();
      await until(()=>latest()?.wheel===name);
      check(latest().simulation.seed===firstSeed && latest().swatches?.length===4,
        name+' changes the film pigments while preserving the existing physical solve');
    }
    set('speed','0');await until(()=>latest()?.simulation?.rate===0);
    set('speed','1.65');set('scale','.55');set('opacity','.41');
    await until(()=>latest()?.simulation?.rate===1.65 && latest()?.detailScale===.55 && latest()?.stack?.[0].opacity===.41);
    click('add-layer');await until(()=>latest()?.stack?.length===baseCount+1);
    const secondSeed=latest().simulation.seed;
    check(secondSeed!==firstSeed,'independent initial conditions on the second track');
    set('speed','.3');set('scale','1.7');set('opacity','.69');
    await until(()=>latest()?.stack?.some(l=>l.simulation?.seed===secondSeed && l.simulation.rate===.3 && l.detailScale===1.7 && l.opacity===.69));
    const other=latest().stack.find(l=>l.simulation.seed===firstSeed);
    check(other.wheel==='meadow' && other.simulation.rate===1.65 && other.detailScale===.55 && other.opacity===.41,
      'editing the second track preserves the first track physics, optics and opacity');
    const picker=$('pigment-colors').querySelector('input');
    picker.value='#ff5522';picker.dispatchEvent(new Event('input'));picker.dispatchEvent(new Event('change',{bubbles:true}));
    await until(()=>latest().swatches[1]===0xff5522);
    check(latest().stack.find(l=>l.simulation.seed===firstSeed).swatches[1]===other.swatches[1], 'editing a blank dye leaves the group independent');
    set('wheel-focus','74');await until(()=>latest().focus===.74);
    check(latest().stack.find(l=>l.simulation.seed===firstSeed).simulation.depth===.23,'global focus and layer depth are independent');
    const authored=await save();sessionStorage.setItem('seeded-project',JSON.stringify(authored));
    const group=authored.layers.find(l=>l.simulationSeed===firstSeed).presetGroup;
    check(authored.layers.filter(l=>l.presetGroup===group).length===baseCount &&
      !Object.hasOwn(authored.layers.find(l=>l.simulationSeed===secondSeed),'presetGroup'),'blank oil is independent of slide recall');
    await reopen(JSON.stringify(authored));
    await until(()=>latest().stack?.some(l=>l.simulation?.depth===.23));
    set('track-source','slide:blue-green');click('add-layer');
    await until(()=>latest().stack?.length===authored.layers.length+recipes.find(r=>r.id==='blue-green').dyes.length);
    const addedSlide=await save();
    check(addedSlide.layers.every(l=>l.simulationSeed!=null) && new Set(addedSlide.layers.map(l=>l.simulationSeed)).size===addedSlide.layers.length,
      'adding a slide fills a new group of independently seeded cells');
    set('slide-preset','yellow-red');await until(()=>latest().wheel==='yellow-red');
    const recolored=await save();
    check(authored.layers.every(l=>JSON.stringify(l)===JSON.stringify(recolored.layers.find(o=>l.id===o.id))),
      'changing a separate slide preserves all authored fields of earlier groups and blank oil');
    const firstAdded=recolored.layers.find(l=>l.id===recolored.selected);
    const sibling=recolored.layers.find(l=>l.presetGroup===firstAdded.presetGroup && l.id!==firstAdded.id);
    $('layer-list').querySelector('[aria-label="Hide '+sibling.name+'"]').click();
    await until(()=>latest().stack?.every(l=>l.layerId!==sibling.id));
    check(latest().stack.find(l=>l.layerId===firstAdded.id).simulation.depth===firstAdded.filmDepth,'hiding a neighboring layer preserves absolute depth');
    set('lamp-a-preset','3200');set('lamp-a-intensity','63');set('layer-projector','b');
    set('lamp-b-kelvin','4675');set('lamp-b-focus','82');click('lamp-b-ghost');
    set('lamp-b-offsetX','15');set('lamp-b-offsetY','-9');
    await until(()=>latest().projectorB?.offsetY===-.09);
    const b=latest().projectorB;
    check(latest().lampKelvin===3200 && latest().projectorAIntensity===.63 && b.lampKelvin===4675 && b.focus===.82 &&
      b.intensity===.08 && b.offsetX===.15 && latest().focus===.74,'independent beam light, exposure, focus and translation');
    check(latest().stack.find(l=>l.layerId===firstAdded.id).projector==='b' &&
      latest().stack.filter(l=>l.layerId!==firstAdded.id).every(l=>!l.projector),'selected film alone routes to the second projector');
    const headings=() => [...$('layer-list').querySelectorAll('[data-projector]')].map(e=>e.textContent);
    check(headings().join('|')==='Projector A · Tungsten · 3200 K|Projector B · 4675 K','group headings reflect each optical source');
    click('lamp-b-enabled');await until(()=>latest().projectorB?.enabled===false);
    check(latest().projectorB.intensity===.08 && latest().stack.some(l=>l.projector==='b'),'muting retains beam B settings and assignments');
    const twoBeams=await save();await reopen(JSON.stringify(twoBeams));
    await until(()=>latest()?.projectorB?.enabled===false);
    check(JSON.stringify(await save())===JSON.stringify(twoBeams),'projector optics and routing round trip');
    click('lamp-b-enabled');await until(()=>latest().projectorB.enabled);
    set('lamp-b-preset','3200');await until(()=>latest().projectorB.lampKelvin===latest().lampKelvin);
    check(headings().length===2 && latest().stack.filter(l=>l.projector==='b').length===1,'identical lamp types remain independent projector groups');
    $('equip-a-options').querySelector('[value=carousel-550r]').click();
    set('equip-a-mode','low');set('equip-a-low','40');
    await until(()=>latest()?.equipmentA?.lampMode==='low' && Math.abs(latest().projectorAIntensity-.63*.4)<1e-8);
    check(latest().equipmentA.lens.focalLengthMm===127 && latest().equipmentA.lens.fNumber===3.5 && !$('equip-a-carousel').hidden,
      'model selection exposes the fitted optical geometry and an effective low lamp mode');
    const beforeAudition=await save();
    check(beforeAudition.projectorAIntensity===.63 && beforeAudition.equipmentA.lowIntensity===.4,'authored exposure and low-lamp estimate are independently preserved');
    const studio=await (await fetch('/test-legend-pack.json')).json();
    const base=studio.rigs[0];
    studio.rigs=[
      {...base,id:'study',artist:'Studio ghost rig',rig:{...base.rig,a:'liquid-overhead',b:'film'}},
      {...base,id:'pair',artist:'Studio twin rig',rig:{...base.rig,a:'liquid-overhead',b:'liquid-overhead'}}
    ];
    async function importPack(data) {
      const transfer=new DataTransfer();
      transfer.items.add(new File([typeof data==='string'?data:JSON.stringify(data)],'study.tsslegends.json',{type:'application/json'}));
      $('legend-file').files=transfer.files;
      $('legend-file').dispatchEvent(new Event('change'));
      await until(()=>!$('legend-install').disabled);
    }
    const keyToGallery=key=>$('legend-carousel').dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true}));
    const activeControl=attr=>$('legend-cards').querySelector('[data-'+attr+']');
    const packControl=(action,id)=>[...$('legend-packs').querySelectorAll('[data-'+action+']')].find(control=>control.dataset[action]===id);
    check(!$('legend-empty').hidden && $('legend-carousel').hidden,'fresh install has an empty optional gallery');
    catalogFiles=[{fileName:'corrupt.tsslegends.json',text:'{'},{fileName:'studio.tsslegends.json',text:JSON.stringify(studio)}];
    $('legends').open=true;
    await until(()=>!!$('legend-available-list').querySelector('button'));
    check($('legend-carousel').hidden && $('legend-catalog-status').textContent.includes('corrupt.tsslegends.json'),
      'discovery lists valid peer packs without activating them and identifies invalid files');
    $('legend-available-list').querySelector('button').click();
    check($('legend-title').textContent==='Studio ghost rig' && $('legend-position').textContent.startsWith('1 of 2') &&
      $('legend-cards').textContent.includes(studio.credits[0]) && $('legend-cards').textContent.includes(studio.review.note),
      'installed entries expose credit and version-specific review');
    const otherPack={...studio,id:'gnpng-other-study',title:'Studio rehearsal',rigs:[{...studio.rigs[0],artist:'Other setup'}]};
    await importPack(otherPack);
    check($('legend-title').textContent==='Other setup' && $('legend-position').textContent.startsWith('3 of 3'),
      'rig identifiers can repeat in independent packs');
    click('legend-next');
    check($('legend-title').textContent==='Studio ghost rig','carousel advances through packs and wraps');
    activeControl('audition').click();
    await until(()=>latest()?.equipmentA?.profile==='liquid-overhead' && latest().projectorB?.equipment?.profile==='film');
    check(JSON.stringify(await save())===JSON.stringify(beforeAudition),'rig audition leaves the project untouched');
    keyToGallery('End');
    check(activeControl('audition').getAttribute('aria-pressed')==='false' && !$('legend-stop').hidden,
      'browsing to a pack with the same rig id leaves the intended audition active');
    packControl('remove',otherPack.id).click();
    check($('legend-title').textContent==='Studio twin rig' && !$('legend-stop').hidden,'removing another pack preserves the audition');
    $('preview').scrollIntoView({block:'center'});click('legend-preview-stop');
    check($('legend-stop').hidden,'stopping from the preview returns to the rig');
    $('preview').scrollIntoView({block:'center'});document.dispatchEvent(new Event('visibilitychange'));
    await until(()=>latest()?.equipmentA?.profile==='carousel-550r');
    keyToGallery('Home');keyToGallery('ArrowRight');
    activeControl('apply').click();
    await until(()=>latest()?.projectorB?.equipment?.profile==='liquid-overhead');
    check((await save()).layers.every((l,i)=>l.simulationSeed===beforeAudition.layers[i].simulationSeed),'applying a historical rig preserves existing oil');
    const appliedRig=await save();
    packControl('remove',studio.id).click();
    check(!$('legend-empty').hidden && JSON.stringify(await save())===JSON.stringify(appliedRig),'removing the pack preserves applied project settings');
    click('undo-edit');await idle();
    check(JSON.stringify(await save())===JSON.stringify(beforeAudition),'applied historical rig is one undoable operation');
    await importPack(studio);activeControl('audition').click();packControl('remove',studio.id).click();
    await until(()=>latest()?.equipmentA?.profile==='carousel-550r');
    check($('legend-stop').hidden,'removing the performing pack ends its audition');
    await importPack(studio);activeControl('audition').click();
    packControl('enable',studio.id).click();
    await until(()=>latest()?.equipmentA?.profile==='carousel-550r');
    check(!$('legend-empty').hidden && !$('legend-installed').hidden && $('legend-stop').hidden &&
      JSON.stringify(await save())===JSON.stringify(beforeAudition),'disabling an auditioned pack restores original project, retaining pack management');
    const revision={...studio,version:'1.2.0',rigs:[{...base,artist:'Studio signed-off rig'}]};
    await importPack(revision);
    check(!$('legend-replacement').hidden && $('legend-carousel').hidden,'duplicate identifiers require explicit replacement');
    click('legend-keep');
    await importPack(revision);click('legend-replace');
    check($('legend-carousel').hidden && !packControl('enable',studio.id).checked,'replacing a disabled pack preserves its disabled state');
    packControl('enable',studio.id).click();
    await importPack('{');
    check($('legend-title').textContent==='Studio signed-off rig' && !!$('legend-pack-status').textContent,
      'failed import leaves the replaced pack intact');
    $('legends').open=false;
    $('preview').scrollIntoView({block:'center'});
    const normalRequests=previewRequests.filter(Boolean);
    check(normalRequests.every(c=>c.simulation && (!c.stack || c.stack.every(l=>l.simulation))),
      'every normal palette and new blank track draws solved fields');
    const legacy=await (await fetch('/coral-wheel.tss')).text();
    await reopen(legacy);
    await until(()=>latest()?.motion==='wheel-with-coral-beads' && !latest().stack);
    check(!latest().simulation && $('wheel-motion').value==='wheel-with-coral-beads','legacy recorded motion survives opening with a solver installed');
    check(JSON.stringify(await save())===JSON.stringify(JSON.parse(legacy)),'legacy project content survives round trip');
    await window.tssImportAudio();
    set('range-start','.25');set('range-end','1.25');click('set-range');
    set('advice-direction','Add an oil cue above the coral study');click('advice-suggest');
    await until(()=>advisorRequests.length>0);
    finishInference();await until(()=>!$('advice-review').hidden);
    set('advice-visual','oil');set('advice-signal','time');set('advice-fit','elastic');
    set('advice-cue','Coral cue');click('advice-apply');
    await until(()=>$('selected-name').textContent==='Coral cue');
    await until(()=>Number.isInteger(latest()?.simulation?.seed));
    const cue=(await save()).layers[0];
    check(cue.motion==='wheel-flow' && Number.isInteger(cue.simulationSeed) && cue.palette==='violet-blue',
      'an authored oil cue inherits the palette of the selected legacy study and saves its own live seed');
    check(latest().stack.some(l=>l.motion==='wheel-with-coral-beads' && !l.simulation) &&
      latest().stack.some(l=>l.simulation?.seed===cue.simulationSeed),'the original recorded layer survives adding the new live oil cue');
    click('advice-suggest');await until(()=>advisorRequests.length===2);finishInference();await until(()=>!$('advice-review').hidden);
    set('advice-visual','oil');set('advice-signal','time');set('advice-fit','elastic');set('advice-cue','Second oil cue');click('advice-apply');
    await until(()=>$('selected-name').textContent==='Second oil cue');
    check((await save()).layers[0].simulationSeed===cue.simulationSeed,'an oil cue based on a seeded layer retains its source seed');

    const templates=[...$('track-source').options].map(o=>o.value).filter(v=>v.startsWith('shipped:'));
    check(templates.includes('shipped:wheel') && templates.includes('shipped:lagoon') &&
      !templates.includes('shipped:beads'),'normal oil authoring offers the live shipped templates');
    const usedSeeds=new Set([cue.simulationSeed]);
    for (const template of templates) {
      set('track-source',template);set('wand-mode','stretch');click('add-layer');await idle();
      const added=(await save()).layers[0];
      check(Number.isInteger(added.simulationSeed) && !usedSeeds.has(added.simulationSeed),
        template+' creates independent simulated oil');
      usedSeeds.add(added.simulationSeed);
    }

    await reopen(legacy);
    set('wheel-motion','simulated');await until(()=>Number.isInteger(latest()?.simulation?.seed));
    check((await save()).layers[0].simulationSeed===latest().simulation.seed,'an explicit legacy-to-live conversion persists its new seed');

    // Restart the real editor with a separate capability response and reopen its saved live project.
    location.hash='missing-helper';location.reload();
    return new Promise(()=>{});
  }
  if (coral || oils) {

    incoming=await (await fetch('/coral-wheel.tss')).text();
    click('open-project');await idle();
    check($('app-status').textContent==='Opened coral-wheel.tss.' && $('track-title').textContent==='coral-wheel.tss','normal project open');
    check($('layer-count').textContent.startsWith('1') && $('selected-name').textContent==='Wheel with coral beads','selected coral wheel');
    check($('slide-preset').value==='violet-blue' && $('wheel-motion').value==='wheel-with-coral-beads' &&
      $('wheel-films').value==='both' && $('wheel-slide').value==='front' && $('wheel-gesture').value==='tug' &&
      $('wheel-focus').value==='90' && $('wheel-width').value==='960' && $('tug-response').value==='0.4','capture controls');
    check($('play').disabled && $('relink-audio').hidden && $('source-name').textContent==='No audio selected','audio-free project');
    await until(()=>previewRequests.at(-1)?.motion==='wheel-with-coral-beads');
    const first=previewRequests.at(-1);
    check(first.layerId===1 && first.wheel==='violet-blue' && first.renderWidth===960 && first.focus===.9 &&
      first.visibleFilms==='both' && first.slide==='front' && first.gesture==='tug' && first.tugResponse===.4,'native configuration');
    click('save-project');await idle();
    check(JSON.stringify(JSON.parse(incoming))===documentText,'unaltered example round trip');
    check(!$('replay-release').disabled && $('replay-release').textContent==='Replay front beads','available front replay');
    click('replay-release');await until(()=>previewRequests.at(-1)?.motionTake===first.motionTake+1);
    check(previewRequests.at(-1).visibleFilms==='both' && $('play').disabled,'silent replay preserves both films');
    if(oils) {
      const rear=$('rear-film-oil'), front=$('front-film-oil');
      check(!rear.disabled && front.disabled && rear.value==='wheel' && front.value==='wheel','legacy coral sources');
      set('wheel-motion','wheel-flow');
      check(!rear.disabled && !front.disabled,'both wheel oils editable');
      set('rear-film-oil','pdms-20');
      await until(()=>previewRequests.at(-1)?.materialMotion?.rear==='pdms-20');
      click('undo-edit');await until(()=>rear.value==='wheel' && !!previewRequests.at(-1) && !previewRequests.at(-1).materialMotion);
      click('redo-edit');await until(()=>rear.value==='pdms-20');
      set('front-film-oil','pdms-1000');
      await until(()=>previewRequests.at(-1)?.materialMotion?.front==='pdms-1000');
      set('wheel-motion','coral-release');
      check(rear.disabled && front.disabled && !$('film-oil-note').hidden,'prescribed coral source overrides both');
      set('wheel-motion','wheel-with-coral-beads');
      check(!rear.disabled && front.disabled && front.value==='pdms-1000','front release retains its authored choice');
      click('save-project');await idle();
      const saved=JSON.parse(documentText),original=JSON.parse(incoming).layers[0];
      check(saved.layers[0].materialMotion.rear==='pdms-20' && saved.layers[0].materialMotion.front==='pdms-1000','saved independent oil choices');
      for(const field of ['palette','colors','opacity','tugResponse','mousePressure'])
        check(JSON.stringify(saved.layers[0][field])===JSON.stringify(original[field]),'oil preserves '+field);
      incoming=documentText;set('rear-film-oil','wheel');
      click('open-project');await until(()=>$('replace-project').open);$('replace-project').querySelector('[value=replace]').click();await idle();
      check(rear.value==='pdms-20' && front.value==='pdms-1000' && front.disabled,'normal reopen restores both oils and source override');
      set('wheel-motion','wheel-flow');click('add-layer');set('slide-preset','yellow-red');
      await until(()=>previewRequests.at(-1)?.stack?.length===2);
      check(rear.disabled && front.disabled && Number.isInteger(previewRequests.at(-1).simulation?.seed),
        'the added yellow/red layer uses the independent live solver');
      const layers=previewRequests.at(-1).stack;
      check(layers.some(l=>l.wheel==='violet-blue' && l.materialMotion.rear==='pdms-20' && l.materialMotion.front==='pdms-1000'),
        'stack keeps the first wheel independent of selected wheel');
    }
    return;
  }
  if (platform==='Macintosh') {
    const tools=$('media-tools'),install=$('media-tools-install');
    check(!tools.hidden && mediaChecks===0 && mediaInstalls===0,'desktop starts without scanning packages');
    click('artwork-add');await until(()=>tools.open && !install.disabled);
    check($('media-tools-manager').textContent==='brew install ffmpeg' && $('media-tools-versions').textContent.includes('Could not find ffmpeg.'),'operator sees exact command and missing executable');
    click('media-tools-install');click('media-tools-install');
    await until(()=>$('media-tools-status').textContent.includes('Package download failed.'));
    check(mediaInstalls===1 && mediaChecks===2,'failed install is rechecked and duplicate click ignored');
    mediaReady=true;click('media-tools-check');await until(()=>$('media-tools-status').textContent==='FFmpeg media tools are ready.');
    check(install.hidden && $('media-tools-versions').textContent.includes('/usr/local/bin/ffprobe') &&
      !$('media-tools-versions').textContent.includes('Could not find'),'recheck clears stale errors and reports local versions');
    tools.open=false;
    await until(()=>previewRequests.some(c=>c?.wheel==='violet-blue'));
    const preview=previewRequests.at(-1);
    check(preview.renderWidth===640 && preview.rect.width>0 && preview.rect.height>0,'Mac uses live wheel and device pixel viewport');
    check(!$('native-wheel-controls').hidden && !$('artwork-add').hidden,'Mac exposes live optics and media controls');
    set('film-depth','82');await until(()=>previewRequests.at(-1)?.simulation?.depth===.82);
    set('wheel-focus','68');await until(()=>previewRequests.at(-1)?.focus===.68);
    click('show-logo');await until(()=>previewRequests.at(-1)===null);
    $('logo-dialog').close();await until(()=>previewRequests.at(-1)?.wheel==='violet-blue');
    check(Number.isInteger(previewRequests.at(-1).simulation?.seed),'Mac desktop defaults to live oil');
    incoming=await (await fetch('/coral-wheel.tss')).text();
    click('open-project');await until(()=>$('replace-project').open);
    $('replace-project').querySelector('[value=replace]').click();await idle();
    set('wheel-films','front');
    set('layer-projector','b');set('lamp-a-preset','3200');set('lamp-b-intensity','19');
    $('equip-b-options').querySelector('[value=carousel-550r]').click();set('equip-b-mode','low');
    await window.tssImportAudio();
    await until(()=>!$('export-native-wheel').disabled);
    click('export-native-wheel');
    await until(()=>movieRequests.length===1 && !$('native-wheel-export-progress').open);
    const movie=movieRequests[0];
    check(movie.request.lampKelvin===3200 && movie.request.stack?.[0].projector==='b' &&
      movie.request.projectorB.equipment?.profile==='carousel-550r' && Math.abs(movie.request.projectorB.intensity-.19*.6)<1e-8,
      'native recorded-field movie receives the auditioned light settings, fitted lens and the effective relative output');
    check(movie.request.visibleFilms==='front' && movie.request.wheel==='violet-blue' &&
      movie.request.duration>1.9 && movie.audioPath.endsWith('two-slide-cues.m4a') &&
      movie.outputPath.endsWith('.mp4'),'Mac movie command carries selected film, decoded duration and chosen paths');
    return;
  }
  const tracks=()=>parseInt($('layer-count').textContent,10), initialTracks=tracks();
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
    tracks()===initialTracks,'pending inference on a stale range cannot alter the arrangement');
  set('range-end','1.25');click('set-range');click('advice-lyrics');click('advice-suggest');
  await until(()=>advisorRequests.length===2);
  check(advisorRequests[1].direction==='Violet at the downbeat\nLyric excerpt: Reviewed lyric' && advisorRequests[1].port===18081,
    'only opted-in reviewed cue text overlapping this range reaches inference');
  finishInference();await until(()=>!$('advice-review').hidden);
  check($('advice-apply').disabled && $('advice-visual').value==='video' && $('advice-fit').value==='repeat' &&
    $('advice-range').textContent==='0.25–1.25 s','review precedes any track change');
  set('advice-visual','oil');set('advice-signal','time');set('advice-fit','elastic');set('advice-cue','Reviewed blue oil');
  click('advice-apply');await until(()=>$('advice-status').textContent.includes('Added the reviewed oil layer'));
  check(tracks()===initialTracks+1 && $('selected-name').textContent==='Reviewed blue oil','edited suggestion adds a real selected track');
  click('save-project');await idle();
  const applied=JSON.parse(documentText).layers[0];
  check(applied.name==='Reviewed blue oil' && applied.envelope.length===1 && applied.envelope[0].start===.25 &&
    applied.envelope[0].end===1.25 && applied.envelope[0].rule.mode==='stretch','granted range and edited fit persisted');
  click('undo-edit');await idle();
  check(tracks()===initialTracks && $('selected-name').textContent!=='Reviewed blue oil','advisor track is undoable');
  click('deselect-time');click('advice-lyrics');
  if (windows) {
    set('film-depth','21');set('wheel-focus','71');
    set('wheel-gesture','pressure');set('mouse-pressure','81');
    set('native-export-baseline','neutral');
  }
  click('add-layer'); set('speed','1.8'); click('save-project');await idle();
  const saved=JSON.parse(documentText);
  check(saved.layers.length===initialTracks+1 && saved.layers[0].speed===1.8 && saved.audio.sha256.length===64,'save controls');
  const first=saved.layers.find(l=>l.id===1);
  if (windows) check(first.visibleFilms==='rear' && first.filmDepth===.21 && saved.focus===.71 && first.mousePressure.rear===.81 &&
    saved.gestures.macros[0].name==='Pulse' && saved.gestures.lyricGroups[0].occurrences.length===2 && saved.exportBaseline==='neutral','native controls');
  click('undo-edit');await idle();check($('speed').value==='1','undo');
  click('redo-edit');await idle();check($('speed').value==='1.8','redo');
  incoming=JSON.stringify({...saved,version:2});click('open-project');await idle();
  check($('app-status').textContent.includes('Unsupported') && tracks()===initialTracks+1,'invalid open preserves session');
  incoming=documentText;click('add-layer');click('open-project');await until(()=>$('replace-project').open);
  $('replace-project').querySelector('[value=keep]').click();await idle();check(tracks()===initialTracks+2,'cancel replacement');
  click('open-project');await until(()=>$('replace-project').open);$('replace-project').querySelector('[value=replace]').click();await idle();
  check(!$('relink-audio').hidden && $('play').disabled && tracks()===initialTracks+1,'offline project');
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
  click('add-layer');await idle();check(tracks()===initialTracks+2,'add template track');
  click('clear-region');await idle();click('save-project');await idle();
  check(JSON.parse(documentText).layers[0].envelope.length===0,'regional clear');
  click('undo-edit');await idle();
  click('merge-region-project');await idle();click('save-project');await idle();
  const merged=JSON.parse(documentText);
  check(merged.layers.length===2*initialTracks+3 && new Set(merged.layers.map(l=>l.id)).size===merged.layers.length && merged.layers[0].envelope[0].start===.25,'regional merge');
  if(windows)check(merged.gestures.macros.length===2 && merged.gestures.macros[1].events[0].layerId===merged.layers.slice(0,saved.layers.length).find(l=>l.name===first.name).id,'regional gesture remap');
  const media={name:'Plate.gif',sha256:'a'.repeat(64),width:200,height:120,durationUs:800000,still:false,
    offsetUs:0,rate:1,loop:true,mode:'color',keyEnabled:false,keyColor:'#00FF00',keyThreshold:.2,keySoftness:.1,
    gradientA:'#000000',gradientB:'#FFFFFF',gradientRate:0,blur:0,distortion:0,bounce:0,maskA:null,maskB:null,
    maskOp:'union',shape:'none',assetId:999};
  merged.layers[0].media=media;
  merged.layers[1].media={...media,name:'Offline.gif',sha256:'b'.repeat(64)};
  for(const layer of merged.layers.slice(0,2)){
    delete layer.simulationSeed;delete layer.filmDepth;delete layer.presetGroup;delete layer.dyeIndex;
    layer.colors=['#ffffff','#aa88aa','#335533','#aaff00'];
  }
  incoming=JSON.stringify(merged);click('open-project');await idle();
  check($('app-status').textContent.includes('Offline.gif'),'offline visual source named: '+$('app-status').textContent);
  click('save-project');await idle();const restored=JSON.parse(documentText);
  check(restored.layers[0].media.width===400 && restored.layers[1].media.width===200 &&
    !restored.layers.some(l=>l.media?.assetId!==undefined),'portable media link, offline metadata and refreshed grant');
}
const chrome=process.env.CHROME_BIN;
const projectControls='editor project and region controls, reviewed advisor cues, lyric consent and undo';
const editorCases=[
  ['Macintosh pressure meter','MacBook measures raw force and applied slide load with adjustable sensitivity'],
  ['Linux','Linux '+projectControls],
  ['Windows','Windows '+projectControls],
  ['Macintosh','Mac desktop native wheel selection, optics and modal visibility'],
  ['Macintosh stereo session','Mac stereo pressure autostart, physical force, loss and project restoration'],
  ['Windows coral project','Open and replay the standalone coral capture project without audio'],
  ['Windows live solver','Desktop live solver lifecycle, independent pigments and seed-preserving offline reopen'],
  ['Windows recorded benchmark','Explicit two-track recorded benchmark remains reproducible'],
  ['Windows film oils','Independent slide oil selection, release overrides, undo, save, reopen and stack'],
];
for (const [platform,title] of editorCases) test(title,
  {skip:!chrome && 'Set CHROME_BIN to a Chrome executable', timeout:platform==='Windows live solver'?45000:30000}, async()=>{
    const web=new URL('../../WebAssets/',import.meta.url);
    const media=new URL('../slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a',import.meta.url);
    const scratch=await mkdtemp(join(tmpdir(),'tss-project-ui-'));
    let report;
    const done=new Promise(resolve=>{report=resolve;});
    const server=createServer(async(req,res)=>{
      try {
        if(req.url==='/report') {let body='';for await(const part of req)body+=part;res.end();report(body);return;}
        if(req.url.startsWith('/assets/wheel-previews/') || /\.mp4(?:\?|$)/.test(req.url)){
          res.writeHead(404);res.end();report('Unexpected video fetch: '+req.url);return;
        }
        if(req.url==='/') {
          const html=(await readFile(new URL('index.html',web),'utf8')).replace('<script type="module" src="app.js"></script>',
            '<script type="module">('+exerciseControls.toString()+')('+JSON.stringify(platform)+').then(()=>fetch("/report",{method:"POST",body:"pass"})).catch(e=>fetch("/report",{method:"POST",body:e.stack}));</script>');
          res.writeHead(200,{'Content-Type':'text/html'});res.end(html);return;
        }
        if(req.url==='/coral-wheel.tss') {
          res.writeHead(200,{'Content-Type':'application/json'});
          res.end(await readFile(new URL('../slide-look-study/approval/coral-wheel/coral-wheel.tss',import.meta.url)));return;
        }
        if(req.url==='/test-legend-pack.json'){
          res.writeHead(200,{'Content-Type':'application/json'});
          res.end(await readFile(new URL('../../examples/legend-packs/two-beam-studio.tsslegends.json',import.meta.url)));return;
        }
        if(req.url==='/sample.m4a'){res.writeHead(200,{'Content-Type':'audio/mp4'});res.end(await readFile(media));return;}
        const allowed=/^\/(?:[a-z-]+\.js|styles\.css|assets\/(?:slide-presets\.json|visual-advice\.json|tss-project-logo\.jpeg))$/;
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
      const timeout=new Promise(resolve=>{timer=setTimeout(()=>resolve('Browser timed out'),platform==='Windows live solver'?40000:24000);});
      assert.equal(await Promise.race([done,timeout,exited.then(()=> 'Browser exited')]),'pass');
    } finally {
      clearTimeout(timer);proc.kill();await exited;server.closeAllConnections();server.close();
      await rm(scratch,{recursive:true,force:true,maxRetries:5,retryDelay:50});
    }
  });
