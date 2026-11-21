import test from 'node:test';
import assert from 'node:assert/strict';
import {advisorIntervals,beatObservations,mapEnvelope,opacityAt,planIntervals,replaceRegion,validEnvelope} from '../../WebAssets/timeline.js';
const params={length:2,interval:3,fadeIn:1,fadeOut:.5,level:.8,autoScale:true,tolerance:50,gap:.05,signal:'bass',threshold:.4};
const range={start:3,end:10};
test('clear and remap retain the shape of an existing ramp at cut boundaries',()=>{
  const original=[{start:0,end:10,fadeIn:8,fadeOut:1,level:.8}];
  const kept=replaceRegion(original,{start:2,end:4},[],10);
  assert.ok(validEnvelope(kept));
  for(const t of [0,1,1.99,4,5,9,9.7])assert.equal(opacityAt(kept,t),opacityAt(original,t));
  for(const t of [2,3.99,10])assert.equal(opacityAt(kept,t),0);
  const moved=mapEnvelope(original,{start:4,end:6},{start:0,end:1},10);
  assert.ok(validEnvelope(moved));assert.ok(moved[0].sourceStart<0);
  assert.equal(opacityAt(moved,.5),opacityAt(original,5));
});
test('fixed, elastic, repeat, recurring and signal matches respect the selected region',()=>{
  const plan=(mode,context={})=>planIntervals({...params,mode,cue:'line'},range,context);
  const times=c=>c.map(({start,end})=>[start,end]);
  assert.deepEqual(times(plan('fixed')),[[3,5]]);
  assert.deepEqual(times(plan('stretch')),[[3,10]]);
  assert.deepEqual(times(plan('repeat')),[[3,5],[6,8],[9,10]]);
  assert.deepEqual(times(plan('recurrence',{groups:[{id:'line',occurrences:[{start:1.5,end:4},{start:6,end:8.4}]}]})),[[3,4],[6,8.4]]);
  const frames=Array.from({length:11},(_,i)=>({bass:i>=5&&i<7?.8:0}));
  assert.deepEqual(times(plan('signal',{analysis:{fps:1,frames}})),[[5,7]]);
});
test('unmatched advisor signal does not grant the remainder of a song',()=>{
  assert.throws(()=>advisorIntervals({...range,fit:'elastic',signal:'beat'},params,[],{fps:30,frames:[]}),/No matching/);
  assert.throws(()=>advisorIntervals({...range,fit:'recurrence',signal:'lyric'},params,[],null),/Choose a recurring/);
  const frames=Array.from({length:240},(_,i)=>({energy:i%15===1?.8:.1,bass:i%15===1?.9:.1}));
  const observations=beatObservations({fps:30,frames});
  assert.equal(observations.length,16);assert.equal(observations[1].start-observations[0].start,.5);
});
test('independent endpoint matching cannot move either endpoint beyond the selected range',()=>{
  const groups=[{id:'line',kind:'lyric',occurrences:[{start:2.9,end:4},{start:3.18,end:4.1},{start:9,end:9.89}]}];
  const r={...params,mode:'stretch',cue:'line',startAnchor:'lyric',endAnchor:'lyric',anchorTolerance:.2};
  const [clip]=planIntervals(r,range,{groups});assert.equal(clip.start,3.18);assert.equal(clip.end,9.89);
  assert.equal(planIntervals({...r,endAnchor:'time'},range,{groups})[0].end,10);
  assert.throws(()=>planIntervals({...r,anchorTolerance:.02},range,{groups}),/within tolerance/);
});
test('transferring and splitting a video excerpt retain the absolute source frame',()=>{
  const media={offsetUs:500000,rate:2};
  const moved=mapEnvelope(null,{start:2,end:4},{start:0,end:4},4,media);
  assert.equal(moved[0].mediaStartUs,4500000);assert.equal(moved[0].mediaRate,1);
  const cut=replaceRegion(moved,{start:1,end:2.5},[],4);
  assert.equal(cut[1].mediaStartUs,7000000);assert.ok(validEnvelope(cut));
  assert.throws(()=>mapEnvelope(null,{start:2,end:4},{start:0,end:.25},4,media),/8×/);
});
