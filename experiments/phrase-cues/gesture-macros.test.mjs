import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initMacroDeck, scaledGesture, songFingerprint, songGestureEvents } from '../../WebAssets/gesture-macros.js';

class Control {
  constructor(){ this.listeners = new Map(); this.children = []; }
  addEventListener(event, callback){ this.listeners.set(event,callback); }
  replaceChildren(...children){ this.children = children; }
  append(...children){ this.children.push(...children); }
  fire(event){ return this.listeners.get(event)?.({target:this}); }
}

test('native capture, cue playback, overlap, mute, seek, and per-song recall', async () => {
  const nodes = new Map(), saved = new Map(), calls=[];
  globalThis.document = {
    getElementById(id){ if(!nodes.has(id)) nodes.set(id,new Control()); return nodes.get(id); },
    createElement(){ return new Control(); }
  };
  globalThis.localStorage = {getItem:k=>saved.get(k) ?? null, setItem:(k,v)=>saved.set(k,v)};
  globalThis.Option = class {constructor(label,value){Object.assign(this,{label,value});}};
  let receiver,slide='rear',origin=100,position=0,playing=false;
  let hold,reply;
  const deck=initMacroDeck({
    listen:async (name,callback)=>{ assert.equal(name,'tss-live-gesture');receiver=callback; },
    invoke:async (name,args)=>{
      if(name==='set_live_wheel_capture_enabled'){
        const payload={layerId:1, slide,gesture:'tug',tugResponse:.4};
        if(args.enabled) {
          receiver({payload:{...payload,samples:[{timeMs:origin+200,phase:'down',point:[.2,.4]}]}});
          receiver({payload:{...payload,layerId:2,samples:[{timeMs:origin+300,phase:'move',point:[0,0]}]}});
          receiver({payload:{...payload,samples:[{timeMs:origin+600,phase:'move',point:[.4,.5]}]}});
          return origin;
        }
        receiver({payload:{...payload,samples:[{timeMs:origin+900,phase:'up',point:[.4,.5]}]}});
        return origin+1000;
      }
      assert.equal(name,'replay_live_wheel_gesture');
      calls.push(args.batch);
      if(hold){hold=false;await new Promise(resolve=>{reply=resolve;});}
    },
    clock:()=>({position,playing}), selection:()=>({layerId:1, slide}), seek:s=>{position=s;deck.transportChanged();}
  });
  const el=id=>document.getElementById('macro-'+id);
  const settle=()=>new Promise(resolve=>setTimeout(resolve,0));
  await deck.songLoaded('song',null,()=>true);
  deck.importSidecar(['verse.vtt','WEBVTT\n\n00:01.000 --> 00:02.000\nturn the wheel\n\n00:03.000 --> 00:04.000\nturn the wheel']);
  for(const side of ['rear','front']){
    slide=side;
    const armed=el('record').fire('click');
    if(side==='front') await deck.finishPendingRecording();
    else {await armed;await el('record').fire('click');}
    el('cue').value='lyric-turn the wheel';el('cue').fire('change');
    el('step').valueAsNumber=.5;el('step').max='3';el('step').fire('input');
    origin+=5000;
  }
  let takes=JSON.parse(saved.get('tss.gestures.v1.song')).macros;
  assert.equal(takes.length,2);
  for(const take of takes){
    assert.deepEqual(take.events.map(e=>e.t),[.2,.6,.9]);
    assert.equal(take.duration,1);
    assert.ok(take.events.every(e=>e.layerId===1));
  }
  playing=true;deck.transportChanged();position=1.01;deck.tick();position=1.65;deck.tick();await settle();
  assert.deepEqual(calls.flatMap(b=>b.samples.map(s=>[b.slide,s.phase])),[
    ['rear','down'],['front','down'],['rear','move'],['front','move']]);
  el('on').checked=false;el('on').fire('change');await settle();
  assert.equal(calls.at(-1).slide,'front');assert.equal(calls.at(-1).samples[0].phase,'up');
  position=1.95;deck.tick();await settle();
  position=3.65;deck.tick();await settle();
  const moved=calls.at(-1).samples.at(-1);
  assert.ok(Math.abs(moved.point[0]-.5)<1e-8);
  assert.ok(Math.abs(moved.point[1]-.55)<1e-8);
  position=0;deck.transportChanged();await settle();
  assert.equal(calls.at(-1).samples[0].phase,'up');
  hold=true;position=1.65;deck.tick();await settle();
  assert.equal(typeof reply,'function');
  position=0;deck.transportChanged();reply();await settle();
  assert.equal(calls.at(-1).samples[0].phase,'up');
  const count=calls.length;
  position=5;deck.transportChanged();deck.tick();await settle();assert.equal(calls.length,count);
  await deck.songLoaded('different-song',null,()=>true);assert.equal(el('editor').hidden,true);
  await deck.songLoaded('song',null,()=>true);assert.equal(el('list').children.length,2);
});

test('weighted rotation crosses the branch cut in slide coordinates', () => {
  const point=a=>[.5+.2*Math.cos(a)*9/16,.5+.2*Math.sin(a)];
  const events=[3.0,-3.0,-2.8].map((a,i)=>({point:point(a),gesture:'rotate',phase:i===0?'down':'move'}));
  const actual=scaledGesture(events,1.5).at(-1).point;
  const expected=point(3+1.5*(2*Math.PI-5.8));
  assert.ok(actual.every((n,i)=>Math.abs(n-expected[i])<1e-9));
});

test('a late live tick applies overlapping cues in the same order as a 4K export', async () => {
  const take = (name, gesture, start, times) => ({
    id:name,name,enabled:true,muted:[],duration:0.2,amount:1,step:0,cap:1,fit:false,
    cue:'recording-position',songStart:start,
    events:times.map((t,i) => ({
      layerId:1,slide:'front',gesture,tugResponse:0.4,t,
      phase:i===0?'down':i===times.length-1?'up':'move',
      point:[[.75,.5],[.5,.75],[.25,.5],[.5,.25]][i],
      ...(gesture==='pressure'?{amount:i===times.length-1?0:0.7}:{})
    }))
  });
  const takes = [
    take('first pressure','pressure',1,[0,.08,.12,.20]),
    take('rotate glass','rotate',1,[0,.05,.10,.20]),
    take('replace pressure','pressure',1.1,[0,.04,.2])
  ];
  const nodes=new Map(), calls=[];
  globalThis.document={
    getElementById(id){if(!nodes.has(id)) nodes.set(id,new Control());return nodes.get(id);},
    createElement(){return new Control();}
  };
  globalThis.Option=class {};
  globalThis.localStorage={getItem:()=>JSON.stringify({macros:takes}),setItem(){}};
  let position=0;
  const deck=initMacroDeck({listen:async()=>{},selection:()=>({layerId:1}),seek(){},
    clock:()=>({position,playing:true}),
    invoke:async(_, {batch})=>calls.push(...batch.samples.map(s=>({gesture:batch.gesture,...s})))
  });
  for (let order=0;order<2;++order) {
    if (order) [takes[0],takes[1]]=[takes[1],takes[0]];
    calls.length=0;position=0;
    await deck.songLoaded('order-'+order,null,()=>true);
    deck.transportChanged();
    position=1;deck.tick();deck.tick(); // A cue on the exact frame boundary fires once.
    position=1.32;deck.tick(); // Cross the replacement and both complete contact windows.
    await new Promise(resolve=>setTimeout(resolve,0));
    const exported=deck.exportEvents(1,2).map(({gesture,phase,point,amount})=>({
      gesture,phase,point,...(gesture==='pressure'?{amount}:{})
    }));
    assert.deepEqual(calls,exported);
  }
});

test('song state follows audio contents',async()=>{
  const encode=s=>new TextEncoder().encode(s);
  assert.equal(await songFingerprint(encode('abc')),'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
});

test('offline performance uses the recording position, independent slides and collision release',()=>{
  function take(id,slide,cue,songStart) {
    const events=['down','move','up'].map((phase,i)=>({phase,point:[.2+i*.1,.3],layerId:7,slide,
      gesture:'tug',tugResponse:.4,t:i}));
    return {id,cue,songStart,enabled:true,fit:true,amount:1,step:.5,cap:1.25,muted:[],events,duration:2};
  }
  const cues=[{id:'verse',occurrences:[{start:1,end:3},{start:4,end:6},{start:7,end:9}]}];
  const rear=take('a','rear','verse',null);
  rear.muted=[1];
  const front=take('b','front','recording-position',1);
  const interrupt=take('c','rear','recording-position',2.5);
  const output=songGestureEvents([rear,front,interrupt],cues,7,9.1);
  const at=(time,slide)=>output.filter(e=>e.time===time && e.slide===slide).map(e=>e.phase);
  assert.deepEqual(at(1,'front'),['down']);
  assert.deepEqual(at(2.5,'rear'),['up','down']);
  assert.deepEqual(at(3,'front'),['up']);
  assert.deepEqual(at(4,'rear'),[]);
  assert.ok(Math.abs(output.find(e=>e.time===8).point[0]-.325)<1e-9);
  assert.deepEqual(songGestureEvents([rear],cues,8,10),[]);
});

test('simultaneous pressure and XY on the same slide preserve contact location, timing and independent amount', () => {
  const id={layerId:3,slide:'front'};
  const xy=(t,phase,x)=>({...id,t,phase,gesture:'tug',point:[x,.6]});
  const pressure=(t,phase,amount)=>({...id,t,phase,gesture:'pressure',point:[.22,.71],amount});
  const take=(events,at=2)=>({id:'take',events,enabled:true,muted:[],duration:1,cue:'recording-position',
    songStart:at,fit:false,amount:1.5,step:0,cap:2});
  const strokes=[xy(0,'down',.1),pressure(.1,'down',.4),xy(.2,'move',.3),pressure(.3,'move',.9),
    xy(.4,'up',.3),pressure(.9,'up',0)];
  const output=songGestureEvents([take(strokes)],[],3,5);
  assert.deepEqual(output.filter(e=>e.gesture==='pressure').map(e=>[e.time,e.amount,e.point]),[
    [2.1,.4*1.5,[.22,.71]], [2.3,1,[.22,.71]], [2.9,0,[.22,.71]]]);
  assert.ok(Math.abs(output.find(e=>e.phase==='move'&&e.gesture==='tug').point[0]-.4)<1e-10);
  const separate=songGestureEvents([take(strokes.filter(e=>e.gesture==='tug')),
      take(strokes.filter(e=>e.gesture==='pressure'),2.1)],[],3,5);
  assert.equal(separate.filter(e=>e.phase==='up').length,2);
  assert.equal(separate.find(e=>e.gesture==='tug'&&e.phase==='up').time,2.4);
});

test('a timed stationary press records and releases without requiring drag motion',async()=>{
  const nodes=new Map(),saved=new Map(),calls=[];
  globalThis.document={getElementById(id){if(!nodes.has(id))nodes.set(id,new Control());return nodes.get(id);},createElement(){return new Control();}};
  globalThis.localStorage={getItem:k=>saved.get(k),setItem:(k,v)=>saved.set(k,v)};
  globalThis.Option=class{};
  let receive;
  const deck=initMacroDeck({listen:async (_,cb)=>{receive=cb;},selection:()=>({layerId:4}),clock:()=>({position:1,playing:true}),
    invoke:async (name,arg)=>{
      if(name==='replay_live_wheel_gesture'){calls.push(arg);return;}
      if(arg.enabled) {
        receive({payload:{layerId:4,slide:'rear',gesture:'pressure',samples:[{phase:'down',timeMs:1010,amount:.8,point:[.37,.42]}]}});
        return 1000;
      } return 1500;
    },seek(){}});
  const record=document.getElementById('macro-record');
  await record.fire('click');
  await deck.finishPendingRecording();
  const [take]=JSON.parse(saved.get('tss.gestures.v1.unsaved-session')).macros;
  assert.deepEqual(take.events.map(e=>[e.t,e.phase,e.amount]),[[.01,'down',.8],[.5,'up',0]]);
  assert.deepEqual(deck.exportEvents(4,3).map(e=>[e.time,e.phase,e.amount]),[[1.01,'down',.8],[1.5,'up',0]]);
});
