import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createGestureInput} from './gesture-input.mjs';

class Surface extends EventTarget {
  constructor() { super(); this.captured = new Set(); this.ownerDocument = {defaultView: {MouseEvent: {WEBKIT_FORCE_AT_MOUSE_DOWN: 1, WEBKIT_FORCE_AT_FORCE_MOUSE_DOWN: 2}}}; }
  getBoundingClientRect() { return {left: 10, top: 20, width: 200, height: 100}; }
  setPointerCapture(id) { this.captured.add(id); }
  hasPointerCapture(id) { return this.captured.has(id); }
  releasePointerCapture(id) { this.captured.delete(id); }
}
function event(type, props = {}) {
  const value = new Event(type, {cancelable: true});
  Object.assign(value, props);
  return value;
}
function pointer(type, props = {}) {
  return event(type, {pointerId: 9, pointerType: 'mouse', button: 0, clientX: 110, clientY: 70, pressure: .5, ...props});
}
test('WebKit force events update the existing mouse stroke and release capture', () => {
  const surface = new Surface(); const records = [];
  const input = createGestureInput(surface, {onGesture: (record) => records.push(record), getTime: () => 12, getLayerIds: () => ['cyan']});
  const willBegin = event('webkitmouseforcewillbegin'); surface.dispatchEvent(willBegin);
  assert.equal(willBegin.defaultPrevented, true);
  surface.dispatchEvent(pointer('pointerdown'));
  assert.equal(records[0].pressure, null); // Mouse .5 is not evidence of measured pressure.
  surface.dispatchEvent(event('webkitmouseforcechanged', {webkitForce: 1.5, clientX: 210, clientY: 120}));
  assert.deepEqual(records[1], {time: 12, sourceId: 'local', pointerId: 9, pointerType: 'mouse', phase: 'move', x: 1, y: 1, pressure: .5, pressureSource: 'webkitForce', rawForce: 1.5, tiltX: null, tiltY: null, layerIds: ['cyan']});
  surface.dispatchEvent(pointer('pointerup'));
  assert.equal(records[2].phase, 'up'); assert.equal(records[2].pressure, 0);
  assert.equal(surface.hasPointerCapture(9), false);
  surface.dispatchEvent(event('webkitmouseforcechanged', {webkitForce: 2}));
  assert.equal(records.length, 3);
  input.dispose();
});
test('unavailable force thresholds retain raw force without claiming normalized pressure', () => {
  const surface = new Surface(); surface.ownerDocument.defaultView.MouseEvent = {};
  const records = []; createGestureInput(surface, {onGesture: (r) => records.push(r)});
  surface.dispatchEvent(pointer('pointerdown'));
  surface.dispatchEvent(event('webkitmouseforcechanged', {webkitForce: 1.25}));
  assert.equal(records[1].pressure, null); assert.equal(records[1].rawForce, 1.25);
});
test('pen pressure, tilt, coalesced samples and cancellation retain a single pointer', () => {
  const surface = new Surface(); const records = [];
  createGestureInput(surface, {onGesture: (r) => records.push(r), getTime: () => 2});
  surface.dispatchEvent(pointer('pointerdown', {pointerType: 'pen', pressure: .25, tiltX: 10, tiltY: -20}));
  const sample = pointer('pointermove', {pointerType: 'pen', pressure: .8, tiltX: 20, tiltY: -10, clientX: 410});
  surface.dispatchEvent(pointer('pointermove', {pointerType: 'pen', getCoalescedEvents: () => [sample]}));
  surface.dispatchEvent(pointer('pointercancel', {pointerType: 'pen'}));
  assert.deepEqual(records.map((r) => r.phase), ['down', 'move', 'cancel']);
  assert.equal(records[1].pressure, .8); assert.equal(records[1].x, 1); assert.equal(records[1].tiltX, 20);
  assert.equal(records[2].pressure, 0); assert.equal(surface.hasPointerCapture(9), false);
});
test('lost capture ends the stroke once; disposal detaches listeners', () => {
  const surface = new Surface(); const records = [];
  const input = createGestureInput(surface, {onGesture: (r) => records.push(r)});
  surface.dispatchEvent(pointer('pointerdown'));
  surface.dispatchEvent(pointer('lostpointercapture'));
  surface.dispatchEvent(pointer('pointerup'));
  assert.deepEqual(records.map((r) => r.phase), ['down', 'cancel']);
  input.dispose(); surface.dispatchEvent(pointer('pointerdown'));
  assert.equal(records.length, 2);
});
test('concurrent pointers preserve separate identifiers, targets and termination', () => {
  const surface = new Surface(); const records = [];
  createGestureInput(surface, {sourceId: 'ipad-1', onGesture: (r) => records.push(r), getLayerIds: (event) => [event.pointerId === 1 ? 'pink' : 'blue']});
  surface.dispatchEvent(pointer('pointerdown', {pointerId: 1, pointerType: 'pen', pressure: .4}));
  surface.dispatchEvent(pointer('pointerdown', {pointerId: 2, pointerType: 'touch', pressure: .3}));
  surface.dispatchEvent(pointer('pointerup', {pointerId: 1, pointerType: 'pen'}));
  surface.dispatchEvent(pointer('pointermove', {pointerId: 2, pointerType: 'touch', pressure: .6}));
  assert.deepEqual(records.map(({sourceId, pointerId, phase, layerIds}) => [sourceId, pointerId, phase, layerIds]), [
    ['ipad-1', 1, 'down', ['pink']], ['ipad-1', 2, 'down', ['blue']],
    ['ipad-1', 1, 'up', ['pink']], ['ipad-1', 2, 'move', ['blue']],
  ]);
  assert.equal(records[3].pressure, .6);
});
