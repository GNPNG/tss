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
test('malformed nested fields, duplicate IDs and unknown versions fail before installation', () => {
  for (const corrupt of [
    p=>p.version=2,p=>p.audio.name='../song.m4a',p=>p.audio.sha256=null,p=>p.selected=99,p=>p.layers[1].id=7,
    p=>p.layers[0].visibleFilms='broken',p=>p.layers[0].colors[2]='orange',p=>p.layers[1].tugResponse.rear=NaN,
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
