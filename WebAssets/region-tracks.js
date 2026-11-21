import {clipInterval,mapEnvelope,validRange} from './timeline.js';
import {scaledGesture} from './gesture-macros.js';

function runs(m, groups) {
  const hits = m.cue==='recording-position' && Number.isFinite(m.songStart)
    ? [{start:m.songStart,end:m.songStart+m.duration}] : groups.find(g=>g.id===m.cue)?.occurrences??[];
  return hits.map((hit,i)=>({start:hit.start,end:hit.start+(m.fit?hit.end-hit.start:m.duration),index:i}));
}

function copyRun(m,run,source,target,layerId,id) {
  const start=Math.max(run.start,source.start),end=Math.min(run.end,source.end);
  if(end<=start)return null;
  const scale=(target.end-target.start)/(source.end-source.start);
  const placement=target.start+(start-source.start)*scale, duration=(end-start)*scale;
  const amount=Math.min(m.cap,m.amount+run.index*m.step);
  const held=new Map(), events=[];
  const channel=e=>e.slide+'/'+(e.gesture==='pressure'?'pressure':'xy');
  const samples=scaledGesture(m.events,amount);
  let entered=false;
  const enter=()=>{
    if(entered)return;
    entered=true;
    for(const e of held.values())events.push({...e,layerId,phase:'down',t:0});
  };
  for(const e of samples) {
    const t=run.start+e.t*(run.end-run.start)/m.duration;
    if(t>=start)enter();
    if(t>end)break;
    if(t>=start)events.push({...e,layerId,t:Math.min(duration,(t-start)*scale)});
    if(e.phase==='up')held.delete(channel(e));else held.set(channel(e),e);
  }
  enter();
  for(const e of held.values())events.push({...e,layerId,t:duration,phase:'up',...(e.gesture==='pressure'?{amount:0}:{})});
  if(!events.length)return null;
  // Each copied occurrence remains independently editable, enabled and muted.
  return {id,name:m.name,enabled:m.enabled,events,duration,songStart:placement,cue:'recording-position',fit:false,
    amount:1,cap:2,step:0,muted:m.muted.includes(run.index)?[0]:[]};
}

// A region stores the resolved positions of acoustic and lyric observations. A user
// can transfer their performance to another recording without running its detector.
export function transferRegion(doc,source,target,layerIds,idFor,prefix) {
  if(!validRange(source)||!validRange(target))throw Error('Choose nonempty source and destination regions.');
  const layers=[],mappedIds=new Map();
  for(const l of doc.layers)if(layerIds.has(l.id)) {
    const id=idFor(l.id);
    mappedIds.set(l.id,id);
    layers.push({...structuredClone(l),id,envelope:mapEnvelope(l.envelope,source,target,source.end)});
  }
  const groups=[...doc.gestures.audioGroups,...doc.gestures.lyricGroups];
  const scale=(target.end-target.start)/(source.end-source.start);
  const map=t=>target.start+(t-source.start)*scale;
  const groupCopy=g=>{
    const occurrences=g.occurrences.flatMap(hit=>{
      const span=clipInterval(hit,source);
      return span?[{...hit,start:map(span.start),end:map(span.end)}]:[];
    });
    return occurrences.length?[{...g,id:prefix+g.id,occurrences}]:[];
  };
  const macros=[];
  for (const m of doc.gestures.macros) {
    const layer=mappedIds.get(m.events[0].layerId);
    if(layer==null)continue;
    if(!m.cue) {
      macros.push({...structuredClone(m),id:prefix+m.id,songStart:null,events:m.events.map(e=>({...e,layerId:layer}))});
      continue;
    }
    for(const run of runs(m,groups)) {
      const copy=copyRun(m,run,source,target,layer,prefix+macros.length+'-'+m.id);
      if(copy)macros.push(copy);
    }
  }
  return {layers,gestures:{macros,selected:macros[0]?.id??null,
    audioGroups:doc.gestures.audioGroups.flatMap(groupCopy),lyricGroups:doc.gestures.lyricGroups.flatMap(groupCopy)}};
}

export function removeRegionGestures(data,layerId,range,duration,prefix) {
  const groups=[...data.audioGroups,...data.lyricGroups],macros=[];
  for(const m of data.macros) {
    const instances=runs(m,groups);
    if(m.events[0].layerId!==layerId||!instances.some(r=>r.start<range.end&&r.end>range.start)) {macros.push(m);continue;}
    for(const r of instances)for(const part of [{start:0,end:range.start},{start:range.end,end:duration}]) {
      if(!validRange(part))continue;
      const copy=copyRun(m,r,part,part,layerId,prefix+macros.length+'-'+m.id);
      if(copy)macros.push(copy);
    }
  }
  return {...data,macros,selected:macros.some(m=>m.id===data.selected)?data.selected:(macros[0]?.id??null)};
}
