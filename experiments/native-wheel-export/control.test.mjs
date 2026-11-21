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
