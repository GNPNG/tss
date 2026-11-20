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
test('a one-shot coral export starts at the beginning unless current pose was explicitly selected', () => {
  const layer = {layerId: 2, wheel: 'violet-green', motion: 'coral-release', motionTake: 7};
  assert.deepEqual(wheelMovieRequest(layer, 8, [], 'auto'), {
    ...layer, duration: 8, events: [], baseline: 'neutral'
  });
  assert.equal(wheelMovieRequest(layer, 8, [], 'current').baseline, 'current');
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
