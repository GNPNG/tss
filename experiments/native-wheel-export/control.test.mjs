import test from 'node:test';
import assert from 'node:assert/strict';
import { wheelMovieRequest } from '../../WebAssets/native-wheel-export.js';
test('scheduled takes use the neutral baseline and preserve explicit overrides', () => {
  const layer = {layerId:7,wheel:'violet-blue'};
  const events = [{time:0,phase:'down',point:[0.4,0.4]}];
  assert.deepEqual(wheelMovieRequest(layer,2,events,'auto'),{...layer,duration:2,events,baseline:'neutral'});
  assert.equal(wheelMovieRequest(layer,2,[],'auto').baseline,'current');
  assert.equal(wheelMovieRequest(layer,2,events,'current').baseline,'current');
  assert.equal(wheelMovieRequest(layer,2,[],'neutral').baseline,'neutral');
});

import { newMedia, programLayers } from '../../WebAssets/visual-media.js';
test('a mixed movie carries the two oil choices on the correct wheel through the scene order', () => {
  const a={id:1,visible:true,opacity:1,palette:'violet-blue',motion:'wheel-flow',motionTake:0,materialMotion:{rear:'pdms-20',front:'pdms-1000'}};
  const b={...a,id:2,projector:'b',palette:'yellow-red',materialMotion:{front:'pdms-20'}};
  const art={id:3,visible:true,opacity:.8,media:newMedia({id:10,name:'Picture.png',sha256:'a'.repeat(64),width:2,height:2,durationUs:1,still:true})};
  const stack=programLayers([a,art,b],l=>!l.media);
  const projectorB={enabled:true,lampKelvin:3000,focus:.36,intensity:.08,offsetX:-.1,offsetY:.22};
  const request=wheelMovieRequest({layerId:1,wheel:a.palette,materialMotion:a.materialMotion,stack,
    focus:.8,lampKelvin:9200,projectorAIntensity:.75,projectorB},6,[],'neutral');
  assert.deepEqual(request.projectorB,projectorB);
  assert.deepEqual([request.focus,request.lampKelvin,request.projectorAIntensity],[.8,9200,.75]);
  assert.deepEqual(request.stack.map(l=>l.projector??'a'),['b','a','a']);
  assert.deepEqual(request.materialMotion,a.materialMotion);
  assert.deepEqual(request.stack.map(l=>l.layerId),[2,3,1]);
  assert.deepEqual(request.stack[0].materialMotion,{rear:'wheel',front:'pdms-20'});
  assert.ok(!Object.hasOwn(request.stack[1],'materialMotion'));
  assert.deepEqual(request.stack[2].materialMotion,a.materialMotion);
});

import { NativeWheelExport } from '../../WebAssets/native-wheel-export.js';
test('native export retains the selected optical films with either pose baseline', () => {
  for (const visibleFilms of ['both', 'rear', 'front', 'clear']) {
    const selected = {layerId: 3, wheel: 'violet-blue', visibleFilms};
    for (const baseline of ['current', 'neutral']) {
      assert.equal(wheelMovieRequest(selected, 2, [], baseline).visibleFilms, visibleFilms);
    }
  }
});
test('a one-shot coral export starts at the beginning unless current pose was explicitly selected', () => {
  for (const motion of ['coral-release', 'wheel-with-coral-beads']) {
    const layer = {layerId: 2, wheel: 'violet-green', motion, motionTake: 7};
    assert.deepEqual(wheelMovieRequest(layer, 8, [], 'auto'), {
      ...layer, duration: 8, events: [], baseline: 'neutral'
    });
    assert.equal(wheelMovieRequest(layer, 8, [], 'current').baseline, 'current');
  }
});
test('export flushes recorded points before capturing the schedule or presenting its dialog', async () => {
  const nodes = new Map();
  const node = id => {
    if (!nodes.has(id)) nodes.set(id, {
      value: 'auto', open: false, addEventListener() {}, removeAttribute() {},
      showModal() { this.open = true; }, close() { this.open = false; },
    });
    return nodes.get(id);
  };
  const old = globalThis.document;
  globalThis.document = {getElementById:node};
  const order = [];
  let sent;
  try {
    const ui = new NativeWheelExport({
      invoke: async (command,payload) => {
        if (command === 'choose_export_path') return 'result.mp4';
        assert.equal(command,'export_native_wheel');
        assert.equal(node('native-wheel-export-progress').open,true);
        sent = payload;
      },
      listen: async () => () => {},
      selection: () => ({layerId:3,wheel:'blue-green'}),
      source: () => ({audioPath:'local.m4a',duration:2}),
      finishRecording: async () => { await Promise.resolve(); order.push('flush'); },
      getEvents: () => { order.push('snapshot'); return [{phase:'down',time:0.2}]; },
      pause: () => order.push('pause'), prepare: async () => order.push('prepare'),
      sync: async () => order.push('restore'), status() {},
    });
    await ui.start();
    assert.deepEqual(order,['flush','pause','prepare','snapshot','restore']);
    assert.equal(sent.request.baseline,'neutral');
    assert.equal(sent.audioPath,'local.m4a');
    assert.equal(node('native-wheel-export-progress').open,false);
  } finally { globalThis.document = old; }
});

import { NativeWheelPlates } from '../../WebAssets/native-wheel-plates.js';
test('still plates need no song and preserve focus and film selection through the real UI request', async () => {
  const selection = {layerId: 4, wheel: 'violet-blue', motion: 'wheel-with-coral-beads', motionTake: 6, visibleFilms: 'rear', focus: 0.2};
  const button = {textContent: 'Export', addEventListener() {}};
  const calls = [];
  let progress;
  const ui = new NativeWheelPlates({
    button, selection: () => selection, busy: () => false,
    prepare: async () => calls.push('prepare'), pause: () => calls.push('pause'),
    status() {}, refresh() {},
    listen: async (name, fn) => { assert.equal(name, 'tss-wheel-plate-progress'); progress = fn; return () => calls.push('unlisten'); },
    invoke: async (name, request) => {
      assert.equal(name, 'export_native_wheel_plates');
      assert.deepEqual(request, {selection});
      progress({payload: 'BG.png'});
      progress({payload: 'MID.png'});
      return 'plates';
    },
  });
  assert.equal(await ui.start(), 'plates');
  assert.deepEqual(calls, ['prepare','pause','unlisten']);
  assert.equal(button.textContent, 'Export');
  ui.invoke = async () => null;
  assert.equal(await ui.start(), null);
  assert.deepEqual(calls, ['prepare','pause','unlisten','prepare','unlisten']);
});

test('the selected-wheel export sends print stock and finishing options to the native editor', async () => {
  const selection={layerId:3,wheel:'violet-blue',motion:'wheel',motionTake:1,visibleFilms:{rear:true,front:true},focus:1};
  selection.materialMotion={rear:'pdms-1000',front:'pdms-20'};
  const print={layout:'label7',stock:'uncoated',gilding:'foil',richBlack:true,catalog:6};
  const {NativeWheelPlates}=await import('../../WebAssets/native-wheel-plates.js');
  const button={textContent:'Export',addEventListener(){}};
  const ui=new NativeWheelPlates({button,selection:()=>selection,format:()=>({print}),busy:()=>false,
    prepare:async()=>{},pause(){},status(){},refresh(){},listen:async()=>()=>{},
    invoke:async(name,request)=>{ assert.equal(name,'export_native_wheel_plates');assert.deepEqual(request,{selection:{...selection,print}});return 'mechanical'; }});
  assert.equal(await ui.start(),'mechanical');
});

import {isNativePalette, paletteFields} from '../../WebAssets/film-palette.js';
import {LIQUID_PALETTES} from '../../WebAssets/liquid-renderer.js';
test('all six pigment themes and edited photographic slides reach native stack and solo exports', () => {
  const slides = new Map([['violet-blue', {}]]);
  for (const [id, colors] of Object.entries(LIQUID_PALETTES)) {
    const layer = {id: 1, visible: true, opacity: 1, nativeLive: true, palette: id, hue: 0, colors};
    assert.ok(isNativePalette(layer, slides), id);
    const expected = colors.map(color => Number.parseInt(color.slice(1),16));
    assert.deepEqual(paletteFields(layer).swatches, expected);
    const [item] = programLayers([layer], l => isNativePalette(l,slides));
    assert.deepEqual([item.wheel, item.swatches], [id, expected]);
    const solo = wheelMovieRequest({layerId:1, wheel:id, ...paletteFields(layer)}, 1, [], 'neutral');
    assert.deepEqual(solo.swatches, expected);
    assert.deepEqual(layer.colors, colors);
    assert.equal(isNativePalette({...layer, nativeLive:false},slides),false);
    assert.equal(isNativePalette({...layer, media:{}},slides),false);
  }
  const slide = {palette:'violet-blue', nativeLive:true, colors:['#ffffff','#ff0000','#010203','#0000ff'], hue:0};
  assert.deepEqual(paletteFields(slide), {});
  assert.deepEqual(paletteFields({...slide, customColors:true}).swatches,[0xffffff,0xff0000,0x010203,0x0000ff]);
  assert.equal(paletteFields({...slide,hue:120}).swatches[1],0x00ff00);
});
