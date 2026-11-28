import test from 'node:test';
import assert from 'node:assert/strict';
import { EditHistory, PROJECT_FORMAT, layerSettings, validateProject } from '../../WebAssets/project-file.js';
import {songGestureEvents} from '../../WebAssets/gesture-macros.js';

const palettes = new Set(['orchid', 'violet-blue']);
function fixture(){
  const wheel = {id:1, name:'Violet glass', visible:true, palette:'violet-blue', colors:['#000000','#cabbed','#ffeeee','#00ffff'],
    customColors:false, hue:75, speed:1.5, scale:1.25, opacity:.86, nativeLive:true, motion:'wheel-with-coral-beads',
    motionTake:3, visibleFilms:'front', slide:'rear', gesture:'pressure', tugResponse:{front:0,rear:1},
    mousePressure:{rear:.23,front:.87},renderWidth:960};
  const layer = {...wheel,id:7,name:'Overprint',visible:false,palette:'orchid',nativeLive:false,customColors:true};
  const macro = {id:'take',name:'Press front',duration:2,songStart:1,enabled:true,amount:.8,step:.1,cap:1.9,fit:true,
    cue:'lyric-line',muted:[0],events:['down','move','up'].map((phase,t) => ({phase,t,layerId:1,slide:'front',
      gesture:'pressure',point:[.2,.8],tugResponse:.4,amount:phase==='up'?0:.7}))};
  return {format:PROJECT_FORMAT,version:1,audio:{name:'Original song.m4a',sha256:'a'.repeat(64)},position:10,
    layers:[layer,wheel],selected:1,nextId:9,focus:.61,exportBaseline:'neutral',feedback:{enabled:false,delay:185,strength:73,zoom:97,rotation:-2.5},
    gestures:{macros:[macro],selected:'take',lyricGroups:[{id:'lyric-line',label:'Sing again',kind:'lyric',
      occurrences:[{start:1,end:2},{start:5,end:6}]}],audioGroups:[{id:'audio-a',label:'Mid Mid Low Low',kind:'audio',
      occurrences:[{start:.5,end:2,confidence:.97},{start:3,end:4,confidence:.88}]}]}};
}

test('portable native and procedural layer state, acoustic and lyric triggers round trip', () => {
  const p = fixture();
  for (const l of p.layers) {
    assert.deepEqual(layerSettings({...l,canvas:{},renderer:{}}),l);
  }
  const reopened = validateProject(JSON.parse(JSON.stringify(p)), palettes);
  assert.deepEqual(reopened,p);
  const events = doc => songGestureEvents(doc.gestures.macros,[...doc.gestures.audioGroups,...doc.gestures.lyricGroups],1,12);
  assert.deepEqual(events(reopened),events(p));
  assert.deepEqual(events(reopened).map(e => e.time), [5,5.5,6]);
});
test('independent film oil grades survive project snapshots and default missing sides to the original wheel', () => {
  const doc=fixture(), layer=doc.layers[1];
  assert.ok(!Object.hasOwn(layerSettings(layer),'materialMotion'));
  layer.materialMotion={rear:'pdms-20'};
  assert.deepEqual(layerSettings(validateProject(doc,palettes).layers[1]).materialMotion,{rear:'pdms-20',front:'wheel'});
  layer.materialMotion.front='pdms-1000';
  const restored=validateProject(JSON.parse(JSON.stringify(doc)),palettes);
  assert.deepEqual(restored.layers[1].materialMotion,{rear:'pdms-20',front:'pdms-1000'});
  for(const invalid of [null,[],{front:'20'},{back:'pdms-20'},{rear:1000}]) {
    layer.materialMotion=invalid;
    assert.throws(()=>validateProject(doc,palettes),/layers/);
  }
});

test('one, two and three independently dyed layers retain their focal planes and group slots', () => {
  const color=['#1199aa','#ee2277','#cc9911'];
  for (const count of [1,2,3]) {
    const doc=fixture(), base=doc.layers[1];
    doc.layers=Array.from({length:count},(_,i)=>({...base,id:i+1,motion:'wheel-flow',slide:'rear',visibleFilms:'rear',
      simulationSeed:i*327+1,colors:[color[i]],filmDepth:(i+1)/(count+1),presetGroup:'slide-a',dyeIndex:i}));
    doc.nextId=count+1;
    assert.deepEqual(validateProject({...doc,layers:doc.layers.map(layerSettings)},palettes),doc);
    for (const corrupt of [
      l=>l.filmDepth=-.01,l=>l.filmDepth=1.1,l=>l.filmDepth=null,l=>delete l.simulationSeed,
      l=>l.colors.push('#123456'),l=>l.slide='front',l=>delete l.dyeIndex,l=>l.presetGroup='',
    ]) {
      const bad=structuredClone(doc);corrupt(bad.layers[0]);
      assert.throws(()=>validateProject(bad,palettes),/layers/);
    }
    if (count>1) {
      doc.layers[1].dyeIndex=0;
      assert.throws(()=>validateProject(doc,palettes),/duplicate dye/);
    }
  }
});

test('malformed nested fields, duplicate IDs and unknown versions fail before installation', () => {
  for (const corrupt of [
    p=>p.version=2,p=>p.audio.name='../song.m4a',p=>p.audio.sha256=null,p=>p.selected=99,p=>p.layers[1].id=7,
    p=>p.layers[0].visibleFilms='broken',p=>p.layers[0].colors[2]='orange',p=>p.layers[1].tugResponse.rear=NaN,
    p=>p.equipmentA={profile:'imaginary'},p=>p.equipmentA={profile:'carousel-550r',lens:{focalLengthMm:10,fNumber:16}},
    p=>p.lampKelvin=10001,p=>p.lampKelvin=2699,p=>p.projectorAIntensity=1.1,
    p=>p.projectorB={enabled:true,lampKelvin:6500},p=>p.layers[0].projector='c',p=>p.layers[0].projector='b',
    p=>p.focus=1.1,p=>p.gestures.macros[0].events[1].point=[.5,9],p=>p.gestures.macros[0].events[1].t=3,
    p=>p.gestures.lyricGroups[0].occurrences[1].end=0
  ]) {
    const p=fixture(); corrupt(p); assert.throws(()=>validateProject(p,palettes));
  }
  const empty = {...fixture(),audio:null, layers:[],selected:null,focus:null};
  assert.deepEqual(validateProject(empty,palettes),empty);
});
test('history retains nested state after mutation, branches, drops duplicate steps and bounds memory', () => {
  const history = new EditHistory(), p=fixture();
  history.reset(p,'Initial');
  p.layers.reverse();p.gestures.macros[0].muted=[];history.record(p,'Rearrange');
  assert.deepEqual(history.stateAt(0),fixture());
  assert.deepEqual(history.stateAt(1).layers.map(l=>l.id),[1,7]);
  history.cursor=0;p.focus=.85;history.record(p,'Focus');history.record(p,'Duplicate');
  assert.deepEqual(history.entries.map(e=>e.label),['Initial','Focus']);
  for(let i=0;i<150;i++) {p.position=i;history.record(p,'Move');}
  assert.equal(history.entries.length,128);
  assert.equal(history.stateAt(history.cursor).position,149);
});

test('portable media survives transfer with fresh mask IDs and no session grant',async()=>{
  const {transferRegion}=await import('../../WebAssets/region-tracks.js');
  const {bindProjectMedia}=await import('../../WebAssets/project-media.js');
  const doc=fixture();
  const media={name:'spray.mov',sha256:'f'.repeat(64),width:320,height:180,durationUs:2000000,still:false,
    offsetUs:0,rate:1,loop:true,mode:'key',keyColor:'#00ff00',keyThreshold:.2,keySoftness:.1,gradientA:'#010203',
    gradientB:'#998877',gradientRate:2,bounce:.3,blur:0,distortion:.4,maskA:1,maskB:null,maskOp:'union',shape:'diamond'};
  doc.layers[0].media=media;
  bindProjectMedia(doc.layers[0],{id:303,sha256:media.sha256,width:320,height:180,durationUs:2000000,still:false});
  const portable={...doc,layers:doc.layers.map(layerSettings)};
  assert.equal(portable.layers[0].media.assetId,undefined);validateProject(portable,palettes);
  const copied=transferRegion(portable,{start:1,end:2},{start:3,end:5},new Set([1,7]),id=>id+10,'slice/');
  assert.equal(copied.layers[0].media.maskA,11);assert.equal(copied.layers[0].media.sha256,media.sha256);
  assert.equal(copied.layers[0].envelope[0].mediaStartUs,1000000);
  assert.equal(copied.layers[0].envelope[0].mediaRate,.5);
  assert.equal(copied.gestures.lyricGroups[0].occurrences[0].start,3);
  assert.equal(copied.gestures.macros[0].events[0].layerId,11);
});
