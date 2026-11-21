import { audioCueGroups, lyricCueGroups } from './phrase-cues.js';

const clamp = value => Math.max(0, Math.min(1, value));
const timeText = s => `${Math.floor(s / 60)}:${(s % 60).toFixed(2).padStart(5, '0')}`;
const scalePoint = (point, anchor, amount) => point.map((v, i) => clamp(anchor[i] + amount * (v - anchor[i])));

// XY distances are measured in units of slide height. Keep rotation angle
// amplification continuous across the atan2 branch cut and multi-turn takes.
export function scaledGesture(events, amount) {
  const strokes = new Map();
  return events.map(e => {
    const point = e.point;
    if (e.gesture === 'pressure') return {...e, amount: e.phase === 'up' ? 0 : clamp(e.amount * amount)};
    const key = contactKey(e);
    const angle = Math.atan2(point[1] - .5, (point[0] - .5) * 16 / 9);
    if (e.phase === 'down' || !strokes.has(key)) {
      strokes.set(key,{anchor:point, initialAngle:angle, previousAngle:angle, accumulated:0});
    }
    const stroke=strokes.get(key);
    let next;
    if (e.gesture === 'rotate') {
      const turn = angle - stroke.previousAngle;
      stroke.accumulated += Math.atan2(Math.sin(turn), Math.cos(turn));
      stroke.previousAngle = angle;
      const radius = Math.hypot((point[0] - .5) * 16 / 9, point[1] - .5);
      const a = stroke.initialAngle + amount * stroke.accumulated;
      next = [clamp(.5 + radius * Math.cos(a) * 9 / 16), clamp(.5 + radius * Math.sin(a))];
    } else next = scalePoint(point, stroke.anchor, amount);
    if (e.phase === 'up') strokes.delete(key);
    return { ...e, point: next };
  });
}

export async function songFingerprint(bytes) {
  if (!globalThis.crypto?.subtle) return null;
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
}

const recordedCue = 'recording-position';
const contactKey = e => e.layerId + '/' + e.slide + (e.gesture === 'pressure' ? '/pressure' : '/xy');
const targetsFor = m => new Set(m.events.map(contactKey));
const release = e => ({...e,phase:'up', ...(e.gesture === 'pressure' ? {amount:0} : {})});
const intersects = (a,b) => [...a].some(target => b.has(target));

function cueSchedule(macros, groups) {
  const matches = new Map(groups.map(g => [g.id,g.occurrences]));
  const schedule=[];
  for (const macro of macros) {
    if (!macro.enabled) continue;
    const hits = macro.cue === recordedCue && Number.isFinite(macro.songStart)
      ? [{start:macro.songStart,end:macro.songStart+macro.duration}]
      : matches.get(macro.cue) || [];
    hits.forEach((hit,index) => {
      if (macro.muted.includes(index)) return;
      const duration = macro.fit ? hit.end-hit.start : macro.duration;
      schedule.push({macro, start:hit.start, end:hit.start+duration, hit, index});
    });
  }
  return schedule.sort((a,b)=>a.start-b.start);
}

function firstAtOrAfter(schedule,time) {
  let left=0, right=schedule.length;
  while(left<right) {
    const middle=left+Math.floor((right-left)/2);
    if(schedule[middle].start<time) left=middle+1;
    else right=middle;
  }
  return left;
}

// Export uses the same time fit, amount, hit mutes, and slide ownership as live playback.
export function songGestureEvents(macros,groups,layerId,songDuration) {
  const runs=cueSchedule(macros,groups).filter(r=>r.start<songDuration && r.macro.events[0].layerId===layerId);
  const active=[];
  for (const run of runs) {
    run.targets=targetsFor(run.macro);
    run.cut=songDuration;
    for (let i=active.length-1;i>=0;--i) {
      const prior=active[i];
      if (prior.end<=run.start) active.splice(i,1);
      else if (intersects(prior.targets,run.targets)) {
        prior.cut=run.start;
        active.splice(i,1);
      }
    }
    active.push(run);
  }
  const output=[];
  for(const {macro,start,end,cut,index} of runs) {
    const amount=Math.min(macro.cap,macro.amount+index*macro.step);
    const held=new Map();
    for(const {t,...e} of scaledGesture(macro.events,amount)) {
      const time=start+t*(end-start)/macro.duration;
      if(time>=cut) break;
      output.push({...e,time});
      if(e.phase==='up') held.delete(contactKey(e));
      else held.set(contactKey(e),e);
    }
    if(cut<songDuration) for(const e of held.values()) output.push({...release(e),time:cut});
  }
  return output.sort((a,b)=>a.time-b.time);
}

export function initMacroDeck({ invoke, listen, clock, selection, seek, prepare=async()=>{}, onEdit=()=>{} }) {
  const el = id => document.getElementById('macro-' + id);
  const note = message => { el('status').textContent = message; };
  const all = new Map();
  let key = 'unsaved-session', groups = [], lyricGroups = [], recording = null;
  let projectOwned = false, documentEpoch = 0;
  let selected = null, sequence = Promise.resolve(), previousSongTime = null;
  let captureTask=Promise.resolve();
  let schedule = null, includePreviousStart = true;
  const current = () => all.get(selected);
  const candidates = () => [...groups, ...lyricGroups];
  const save = (checkpoint = true) => {
    schedule = null;
    if (checkpoint) onEdit('Edit gesture macros');
    if (!key || projectOwned) return;
    try { localStorage.setItem('tss.gestures.v1.' + key, JSON.stringify({ macros: [...all.values()], lyricGroups })); }
    catch { note('Storage is full or unavailable. This session still contains the recording.'); }
  };
  function restore() {
    schedule = null;
    all.clear(); lyricGroups = [];
    try {
      const data = JSON.parse(key && localStorage.getItem('tss.gestures.v1.' + key));
      for (const m of data?.macros || []) if (Array.isArray(m.events) && m.events.length && m.duration > 0) all.set(m.id, m);
      if (Array.isArray(data?.lyricGroups)) lyricGroups = data.lyricGroups;
    } catch { note('Could not restore saved gesture macros.'); }
    selected = all.keys().next().value ?? null;
  }
  function render() {
    const m = current();
    const cueItems = candidates();
    el('list').replaceChildren(...[...all.values()].map(m => new Option(m.name, m.id)));
    if (m) el('list').value = m.id;
    el('editor').hidden = !m;
    el('cues').textContent = cueItems.length
      ? cueItems.slice(0, 8).map(g => `${g.label} (${g.occurrences.length})`).join(' · ')
      : 'No recurring cues detected.';
    if (!m) return;
    el('name').value = m.name;
    el('on').checked = m.enabled;
    el('amount').value = m.amount;
    el('amount-value').textContent = Number(m.amount).toFixed(2) + '×';
    el('step').value = m.step;
    el('cap').value = m.cap;
    el('fit').checked = m.fit;
    const recordingOption=Number.isFinite(m.songStart)
      ? [new Option('At recording position · '+timeText(m.songStart),recordedCue)] : [];
    el('cue').replaceChildren(new Option('Manual only', ''), ...recordingOption, ...cueItems.map(g => new Option(`${g.label} · ${g.occurrences.length} hits`, g.id)));
    el('cue').value = m.cue;
    const group = cueItems.find(g => g.id === m.cue);
    el('hits').replaceChildren();
    for (const [i, hit] of (group?.occurrences || []).entries()) {
      const label = document.createElement('label'), check = document.createElement('input'), jump = document.createElement('button');
      check.type = 'checkbox'; check.checked = !m.muted.includes(i);
      check.addEventListener('change', () => { m.muted = m.muted.filter(k => k !== i); if (!check.checked) m.muted.push(i); save(); });
      label.append(check, `Hit ${i+1} `);
      jump.type = 'button'; jump.className = 'button quiet'; jump.textContent = timeText(hit.start);
      jump.title = 'Seek to this occurrence'; jump.addEventListener('click', () => seek(hit.start));
      el('hits').append(label, jump);
    }
  }
  // Each slide has independently owned XY and pressure contacts.
  const players = new Set();
  const retiring = new Set();
  const held = new Map();
  const targetOf = contactKey;
  function enqueue(action) {
    sequence = sequence.then(action).catch(error => note(String(error)));
    return sequence;
  }
  function stopPlayer(player, finishQueued = false) {
    if (player.cancelled) return;
    // Cue replacement preserves samples due before the new cue. An operator
    // stop or seek discards pending samples and releases the actual held contact.
    player.cancelled = !finishQueued;
    players.delete(player);
    retiring.add(player);
    enqueue(async () => {
      try {
        for (const [target, contact] of held) {
          if (contact.player !== player) continue;
          try {
            await invoke('replay_live_wheel_gesture', { batch: {
              ...contact.header, samples: [{ phase:'up',point:contact.point,
                ...(contact.header.gesture==='pressure'?{amount:0}:{}) }]
            } });
          } finally { held.delete(target); }
        }
      } finally { retiring.delete(player); }
    });
  }
  function stop() {
    for (const p of retiring) p.cancelled = true;
    for (const p of players) stopPlayer(p);
  }
  function send(player, header, samples) {
    ++player.pending;
    enqueue(async () => {
      try {
        if (player.cancelled) return;
        await invoke('replay_live_wheel_gesture', { batch: { ...header, samples } });
        const last = samples.at(-1);
        if (last.phase === 'up') held.delete(targetOf(header));
        else held.set(targetOf(header), {player, header, point:last.point});
      } catch (error) {
        stopPlayer(player);
        throw error;
      } finally {
        --player.pending;
        if (!player.pending && player.next === player.events.length) players.delete(player);
      }
    });
  }
  function projectedLayers() {
    const layer = selection();
    return new Set(layer?.stack?.map(l => l.layerId) ?? (layer ? [layer.layerId] : []));
  }
  function startMacro(m, startTime, hit, index = 0) {
    if (recording || !m?.enabled) return;
    const layers = projectedLayers();
    if (m.events.some(e => !layers.has(e.layerId))) {
      if (!hit) note('Select the visual track where this gesture was recorded.');
      return;
    }
    const targets = targetsFor(m);
    for (const other of players) {
      if (intersects(targets,other.targets)) stopPlayer(other, !!hit);
    }
    const amount = Math.min(m.cap, m.amount + m.step * index);
    players.add({ id:m.id, events:scaledGesture(m.events, amount), next:0, start:startTime,
      duration:hit && m.fit ? hit.end - hit.start : m.duration, sourceDuration:m.duration,
      song:!!hit, targets, pending:0, cancelled:false });
    note(`Playing ${m.name} at ${amount.toFixed(2)}×`);
  }
  function pump(until, wallToSong, exclusive = false, active = players) {
    const due = [];
    for (const player of active) {
      const start = player.start + (player.song ? 0 : wallToSong);
      while (player.next < player.events.length) {
        const e = player.events[player.next];
        const time = start + e.t * player.duration / player.sourceDuration;
        if (time > until || (exclusive && time === until)) break;
        due.push({time, player, e});
        ++player.next;
      }
    }
    // Rotating the glass and sampling a pressure center do not commute. Sort
    // across takes first, then batch only adjacent samples for the same command.
    due.sort((a,b) => a.time-b.time);
    let batch;
    for (const {player,e} of due) {
      const {layerId,slide,gesture,tugResponse} = e;
      if (!batch || batch.player !== player || batch.header.slide !== slide ||
          batch.header.gesture !== gesture || batch.header.tugResponse !== tugResponse ||
          batch.samples.length === 512) {
        if (batch) send(batch.player,batch.header,batch.samples);
        batch = {player, header:{layerId,slide,gesture,tugResponse}, samples:[]};
      }
      batch.samples.push({phase:e.phase,point:e.point,...(gesture==='pressure'?{amount:e.amount}:{})});
    }
    if (batch) send(batch.player,batch.header,batch.samples);
  }
  function beginCues(runs,wallToSong,layers) {
    const eligible = runs.filter(r=>r.macro.events.every(e=>layers.has(e.layerId)));
    if (!eligible.length) return;
    const start = eligible[0].start;
    pump(start,wallToSong,true);
    const incoming = new Set(eligible.flatMap(r=>[...targetsFor(r.macro)]));
    // At equal song time, export visits takes in start/insertion order. Visit
    // each original owner in that order, interleaving its release or sample
    // before starting any new take. Pressure changes the coordinates of a tug.
    for (const p of players) {
      const end = p.start+(p.song?0:wallToSong)+p.duration;
      if (end>start && intersects(p.targets,incoming)) stopPlayer(p,true);
      else pump(start,wallToSong,false,[p]);
    }
    for (const r of eligible) startMacro(r.macro,start,r.hit,r.index);
  }
  function tick() {
    const c = clock();
    const wallToSong = c.position - performance.now()/1000;
    const layers = projectedLayers();
    for (const p of players) if (!layers.has(p.events[0].layerId)) stopPlayer(p);
    if (c.playing && previousSongTime !== null && c.position >= previousSongTime && !recording) {
      schedule ??= cueSchedule(all.values(),candidates());
      for (let i=firstAtOrAfter(schedule,previousSongTime);i<schedule.length && schedule[i].start<=c.position;) {
        const first=i++, start=schedule[first].start;
        while (i<schedule.length && schedule[i].start===start) ++i;
        if (includePreviousStart || start!==previousSongTime)
          beginCues(schedule.slice(first,i),wallToSong,layers);
      }
      includePreviousStart = false;
    }
    previousSongTime = c.playing ? c.position : null;
    pump(c.position,wallToSong);
  }
  function finishRecording() {
    const take = recording;
    if (!take) return;
    recording = null;
    el('record').textContent = 'Record gesture';
    if (!take.events.some(e => e.phase === 'move' || e.gesture === 'pressure' && e.phase === 'down' && e.amount > 0)) { note('No gesture was captured.'); return; }
    take.events.sort((a,b)=>a.timeMs-b.timeMs);
    const origin = take.origin ?? take.events[0].timeMs;
    const events = take.events.map(({timeMs, ...e}) => ({...e,t:Math.max(0,(timeMs-origin)/1000)}));
    const last = events.at(-1);
    const open = new Map();
    for (const e of events) {
      if (e.phase==='up') open.delete(contactKey(e));
      else open.set(contactKey(e),e);
    }
    const duration=Math.max(.001,last.t,((take.end ?? origin)-origin)/1000);
    for (const e of open.values()) events.push({...release(e),t:duration});
    const id = globalThis.crypto?.randomUUID?.() ?? 'take-' + Date.now() + '-' + all.size;
    const m = { id, name:`Gesture ${all.size+1}`, events, songStart:take.songStart,
      duration, enabled:true, amount:1, step:0, cap:2, fit:true,
      cue:Number.isFinite(take.songStart)?recordedCue:'', muted:[] };
    all.set(m.id,m); selected=m.id; render(); save(); note(`Saved ${m.name} · ${m.duration.toFixed(2)} s`);
  }
  let listenerError;
  const ready = listen('tss-live-gesture', ({payload}) => {
    if (!recording || payload.layerId !== recording.layerId) return;
    for (const s of payload.samples) {
      recording.events.push({phase:s.phase,point:s.point,timeMs:s.timeMs,layerId:payload.layerId,
        slide:payload.slide,gesture:payload.gesture,tugResponse:payload.tugResponse??.4,
        ...(payload.gesture==='pressure'?{amount:s.amount??0}:{})});
    }
  }).catch(error => { listenerError=error; note(String(error)); });
  async function finishPendingRecording(){
    if (!recording) return;
    const take=recording;
    take.end=await invoke('set_live_wheel_capture_enabled',{enabled:false});
    if(recording===take) finishRecording();
  }
  async function record() {
    el('record').disabled = true;
    try {
      await ready;
      if (listenerError) throw listenerError;
      if (recording) {
        await finishPendingRecording();
      } else {
        const layer = selection();
        if (!layer) { note('Select a live slide first.'); return; }
        await prepare();
        stop();
        await sequence;
        const c=clock();
        recording={layerId:layer.layerId, songStart:c.playing?c.position:null, origin:null, events:[]};
        const take = recording;
        take.origin=await invoke('set_live_wheel_capture_enabled',{enabled:true});
        if (recording === take) {
          el('record').textContent='Finish recording'; note('Recording timed slide gestures.');
        }
      }
    } catch(e) { recording=null; el('record').textContent='Record gesture'; note(String(e)); }
    finally { el('record').disabled=false; }
  }
  el('record').addEventListener('click',()=>{captureTask=record();return captureTask;});
  el('list').addEventListener('change',()=>{selected=el('list').value;render();});
  el('once').addEventListener('click',async()=>{
    try { await prepare(); startMacro(current(),performance.now()/1000,null); }
    catch(error){ note(String(error)); }
  });
  el('name').addEventListener('change',()=>{const m=current();if(m){m.name=el('name').value.trim()||m.name;render();save();}});
  for (const [id,key] of [['amount','amount'],['step','step'],['cap','cap']]) {
    el(id).addEventListener('input',()=>{const m=current();if(!m)return; const v=el(id).valueAsNumber;if(!Number.isFinite(v))return;
      m[key]=Math.min(Number(el(id).max),Math.max(0,v));el('amount-value').textContent=Number(m.amount).toFixed(2)+'×';save(false);});
    el(id).addEventListener('change',()=>onEdit('Set gesture '+id));
  }
  el('on').addEventListener('change',()=>{
    const m=current();
    if (!m) return;
    m.enabled=el('on').checked;
    if (!m.enabled) for (const p of players) if(p.id===m.id) stopPlayer(p);
    save();
  });
  el('fit').addEventListener('change',()=>{if(current()){current().fit=el('fit').checked;save();}});
  el('cue').addEventListener('change',()=>{if(current()){current().cue=el('cue').value;current().muted=[];save();render();}});
  el('lyrics').addEventListener('change',async e=>{
    const file=e.target.files[0]; if(!file)return;
    const epoch=documentEpoch;
    try { const parsed=lyricCueGroups(await file.text(),file.name); if(documentEpoch!==epoch)return;lyricGroups=parsed;render();save();note(`${parsed.length} recurring lyric cues.`); }
    catch(err){note(String(err));}finally{e.target.value='';}
  });
  restore(); render(); el('deck').hidden=false;
  return {
    tick,
    async finishPendingRecording(){await captureTask;await finishPendingRecording();},
    transportChanged(position=clock().position){stop();includePreviousStart=true;previousSongTime=clock().playing?position:null;},
    exportEvents(layerId,duration){return songGestureEvents(all.values(),candidates(),layerId,duration);},
    async songLoaded(id,analysis,stillCurrent){
      stop();
      if(recording) {
        const take=recording;
        try { take.end=await invoke('set_live_wheel_capture_enabled',{enabled:false}); }
        catch(error){ note(String(error)); }
        if(recording===take) finishRecording();
      }
      if(!stillCurrent()) return;
      previousSongTime=null;includePreviousStart=true;
      ++documentEpoch;projectOwned=false;key=id;groups=audioCueGroups(analysis);restore();render();
    },
    projectState(){return {macros:[...all.values()], audioGroups:groups, lyricGroups, selected};},
    async stopForEdit(){await captureTask;await finishPendingRecording();stop();await sequence;},
    restoreProject(data, fromFile=false){
      ++documentEpoch;
      if(fromFile) projectOwned=true;
      schedule=null;previousSongTime=null;includePreviousStart=true;
      all.clear();
      for(const m of data.macros) all.set(m.id,m);
      groups=data.audioGroups;lyricGroups=data.lyricGroups;selected=data.selected;
      render();save(false);
    },
    applyLyricGroups(reviewed) {
      lyricGroups = reviewed;
      render(); save();
    },
    importSidecar(file) {
      if (!file || lyricGroups.length) return;
      const [name,text]=file;
      lyricGroups=lyricCueGroups(text,name);
      render();save();note(`${lyricGroups.length} recurring lyric cues from ${name}.`);
    },
    reportError:error=>note(String(error)),
  };
}
