const number = Number.isFinite;
export const glyphs = ['|---|','[---]','(---)','{---}','*---*'];
export const modes = ['fixed','stretch','repeat','recurrence','signal'];
export const defaultBindings = () => Object.fromEntries(glyphs.map((glyph,i)=>[glyph,modes[i]]));

export function validRange(r) {
  return !!r && number(r.start) && number(r.end) && r.start >= 0 && r.end > r.start;
}
export function validEnvelope(envelope) {
  if (envelope == null) return true;
  if (!Array.isArray(envelope)) return false;
  let previous=0;
  for(const c of envelope) {
    if (!validRange(c) || c.start<previous || !number(c.level) || c.level<0 || c.level>1) return false;
    const a=c.sourceStart??c.start, b=c.sourceEnd??c.end;
    if (!number(a)||!number(b)||a>c.start||b<c.end||!number(b-a)) return false;
    if (![c.fadeIn,c.fadeOut].every(v=>number(v)&&v>=0&&v<=b-a+1e-9)) return false;
    previous=c.end;
  }
  return true;
}
export function opacityAt(envelope,time) {
  if (envelope==null) return 1;
  let low=0, high=envelope.length;
  while(low<high) {
    const mid=Math.floor((low+high)/2);
    if(envelope[mid].end<=time) low=mid+1; else high=mid;
  }
  const c=envelope[low];
  if(!c || time<c.start) return 0;
  return c.level*Math.min(1,c.fadeIn?(time-(c.sourceStart??c.start))/c.fadeIn:1,
    c.fadeOut?((c.sourceEnd??c.end)-time)/c.fadeOut:1);
}
export function clipInterval(clip,range) {
  const start=Math.max(clip.start,range.start), end=Math.min(clip.end,range.end);
  if(end<=start) return null;
  return {...clip,start,end,sourceStart:clip.sourceStart??clip.start,sourceEnd:clip.sourceEnd??clip.end};
}
export const fullEnvelope = duration => [{start:0,end:duration,level:1,fadeIn:0,fadeOut:0}];
export function replaceRegion(envelope,range,replacement,duration) {
  const result=[];
  for(const clip of envelope??fullEnvelope(duration)) {
    if(clip.end<=range.start||clip.start>=range.end) result.push(clip);
    else {
      for(const part of [{start:clip.start,end:range.start},{start:range.end,end:clip.end}]) {
        const kept=clipInterval(clip,part);if(kept)result.push(kept);
      }
    }
  }
  for(const clip of replacement) {const kept=clipInterval(clip,range);if(kept)result.push(kept);}
  return result.sort((a,b)=>a.start-b.start);
}
export function mapEnvelope(envelope,source,target,duration) {
  const scale=(target.end-target.start)/(source.end-source.start);
  const map=t=>target.start+(t-source.start)*scale;
  return (envelope??fullEnvelope(duration)).map(c=>clipInterval(c,source)).filter(Boolean).map(c=>({
    ...c,start:map(c.start),end:map(c.end),sourceStart:map(c.sourceStart),sourceEnd:map(c.sourceEnd),
    fadeIn:c.fadeIn*scale,fadeOut:c.fadeOut*scale
  }));
}

function signalRuns(analysis,range,signal,threshold,gap) {
  if(!analysis?.frames?.length) return [];
  const {frames,fps}=analysis, result=[];
  const first=Math.max(0,Math.floor(range.start*fps)), end=Math.min(frames.length,Math.ceil(range.end*fps));
  for(let i=first;i<end;i++) {
    const f=frames[i];
    const value=signal==='onset'?Math.max(0,f.energy-(frames[i-1]?.energy??0)):f[signal];
    if(value<threshold)continue;
    const start=Math.max(range.start,i/fps), stop=Math.min(range.end,(i+1)/fps);
    const last=result.at(-1);
    if(last&&start-last.end<=gap+1e-9)last.end=stop;
    else result.push({start,end:stop});
  }
  return result;
}
export function planIntervals(rule,range,{groups=[],analysis=null}={}) {
  if(!validRange(range))throw Error('Select a song interval first.');
  const {mode,length,interval,fadeIn,fadeOut,level,autoScale,tolerance}=rule;
  if(!modes.includes(mode)||!number(length)||length<=0||!number(interval)||interval<=0||
    ![fadeIn,fadeOut,tolerance].every(n=>number(n)&&n>=0)||!number(level)||level<0||level>1)throw Error('Check the clip length, interval, fades, tolerance and level.');
  let hits=[];
  if(mode==='fixed')hits=[{start:range.start,end:Math.min(range.end,range.start+length)}];
  if(mode==='stretch')hits=[range];
  if(mode==='repeat') {
    if(interval<length)throw Error('Repeat interval must cover one clip length.');
    if((range.end-range.start)/interval>100000)throw Error('Choose a longer repeat interval.');
    for(let i=0;range.start+i*interval<range.end;i++){
      const start=range.start+i*interval;hits.push({start,end:Math.min(range.end,start+length)});
    }
  }
  if(mode==='recurrence')hits=groups.find(g=>g.id===rule.cue)?.occurrences??[];
  if(mode==='signal') {
    if(!['bass','mid','treble','energy','onset'].includes(rule.signal)||!number(rule.threshold)||rule.threshold<0||
        rule.threshold>1||!number(rule.gap)||rule.gap<0)throw Error('Check the audio signal, threshold and gap.');
    hits=signalRuns(analysis,range,rule.signal,rule.threshold,rule.gap);
  }
  const result=[];
  for(let i=0;i<hits.length;i++) {
    const h=hits[i];
    if(h.end<=range.start||h.start>=range.end)continue;
    const adaptive=mode==='signal'||mode==='recurrence';
    const matched=h.end-h.start;
    const fit=adaptive && autoScale && Math.abs(matched-length)<=length*tolerance/100;
    const sourceEnd=adaptive&&!fit?h.start+length:h.end, span=sourceEnd-h.start;
    const start=Math.max(h.start,range.start), end=Math.min(sourceEnd,range.end,hits[i+1]?.start??Infinity);
    if(end>start)result.push({start,end,sourceStart:h.start,sourceEnd,fadeIn:Math.min(span,fadeIn),
      fadeOut:Math.min(span,fadeOut),level});
  }
  return result;
}

// Report observed attacks on a repeating pulse. The tolerance is measured in
// seconds around one of two adjacent beat multiples; no synthetic hits are emitted.
export function beatObservations(analysis) {
  if(!analysis?.frames?.length)return [];
  const {frames,fps}=analysis;
  const rise=frames.map((f,i)=>Math.max(0,f.bass-(frames[i-1]?.bass??0))+.5*Math.max(0,f.energy-(frames[i-1]?.energy??0)));
  let peak=0; for(const v of rise)peak=Math.max(v,peak);
  const times=[];
  for(let i=1;i+1<rise.length;i++){
    if(rise[i]>=Math.max(.001,.18*peak)&&rise[i]>rise[i-1]&&rise[i]>=rise[i+1]&&(!times.length||i/fps-times.at(-1)>.13))times.push(i/fps);
  }
  let best=0,score=0;
  for(let tempo=60;tempo<=180;tempo++){
    const period=60/tempo;let support=0;
    for(let i=1;i<times.length;i++){
      const gap=times[i]-times[i-1],beats=Math.round(gap/period);
      if(beats<1||beats>2)continue;
      const err=Math.abs(gap-beats*period);
      if(err<.065)support+=(1-err/.065)/beats;
    }
    if(support>score){score=support;best=period;}
  }
  if(score<2 || score<.35*(times.length-1))return [];
  const matches=(a,b)=>{const gap=b-a,k=Math.round(gap/best);return k>=1&&k<=2&&Math.abs(gap-k*best)<.065;};
  return times.filter((t,i)=>matches(times[i-1],t)||matches(t,times[i+1])).map(t=>({start:t,end:t+best}));
}
export function advisorIntervals(s,rule,groups,analysis) {
  if(!['fixed','elastic','repeat','recurrence'].includes(s.fit)||!['time','onset','beat','lyric','energy_rise'].includes(s.signal))
    throw Error('Choose a supported signal and fit.');
  const range={start:s.start,end:s.end};
  let anchors=[range];
  if(s.signal==='lyric' || s.fit==='recurrence') {
    const cue=groups.find(g=>g.id===rule.cue && (s.signal!=='lyric'||g.kind==='lyric'));
    if(!cue)throw Error('Choose a recurring cue in Magic wand, or switch to a manual time boundary.');
    anchors=cue.occurrences;
  } else if(s.signal==='beat') anchors=beatObservations(analysis);
  else if(s.signal!=='time') anchors=signalRuns(analysis,range,'onset',s.signal==='onset'?.01:.035,rule.gap);
  const out=[];
  for(let i=0;i<anchors.length;i++) {
    const a=anchors[i];
    const start=Math.max(range.start,a.start),end=Math.min(range.end,a.end,anchors[i+1]?.start??Infinity);
    if(end<=start)continue;
    out.push(...planIntervals({...rule,mode:s.fit==='elastic'||s.fit==='recurrence'?'stretch':s.fit,
      length:s.fit==='fixed' && s.signal==='time'?end-start:rule.length},{start,end}));
  }
  if(!out.length)throw Error('No matching observations within the selection. Choose another signal or a manual time boundary.');
  return out;
}
