/**
 * Capture a single device-neutral gesture stream from a performance surface.
 * The caller owns timeline time, selected layers, rendering, and persistence.
 */
export function createGestureInput(element, {
  onGesture,
  getTime = (event) => event.timeStamp / 1000,
  getLayerIds = () => [],
  sourceId = 'local',
  preventForceDefault = true,
} = {}) {
  if (!element || typeof element.addEventListener !== 'function' || typeof onGesture !== 'function' || typeof sourceId !== 'string' || !sourceId) {
    throw new TypeError('A surface and onGesture callback are required');
  }
  const active = new Map();
  const listeners = [];
  const view = element.ownerDocument?.defaultView ?? globalThis;
  const clamp = (n, min, max) => Math.max(min, Math.min(max, n));
  const listen = (type, handler) => {
    element.addEventListener(type, handler);
    listeners.push([type, handler]);
  };
  const coordinates = (event, previous) => {
    const rect = element.getBoundingClientRect();
    if (!(rect.width > 0 && rect.height > 0)) return previous ?? {x: 0, y: 0};
    const x = Number.isFinite(event.clientX) ? (event.clientX - rect.left) / rect.width : previous?.x ?? 0;
    const y = Number.isFinite(event.clientY) ? (event.clientY - rect.top) / rect.height : previous?.y ?? 0;
    return {x: clamp(x, 0, 1), y: clamp(y, 0, 1)};
  };
  const penPressure = (event) => event.pointerType !== 'mouse' && Number.isFinite(event.pressure)
    ? clamp(event.pressure, 0, 1) : null;
  const tilt = (event, key) => event.pointerType === 'pen' && Number.isFinite(event[key]) ? event[key] : null;
  function emit(event, state, phase) {
    const time = getTime(event);
    if (!Number.isFinite(time)) throw new TypeError('getTime must return a finite time in seconds');
    onGesture({time, sourceId, pointerId: state.pointerId, pointerType: state.pointerType, phase,
      x: state.x, y: state.y, pressure: state.pressure, pressureSource: state.pressureSource,
      rawForce: state.rawForce, tiltX: state.tiltX, tiltY: state.tiltY, layerIds: [...state.layerIds]});
  }
  listen('pointerdown', (event) => {
    if (active.has(event.pointerId) || (event.pointerType === 'mouse' && event.button !== 0)) return;
    const pressure = penPressure(event);
    const layerIds = getLayerIds(event);
    if (!Array.isArray(layerIds)) throw new TypeError('getLayerIds must return an array');
    const state = {pointerId: event.pointerId, pointerType: event.pointerType,
      ...coordinates(event), pressure, pressureSource: pressure === null ? null : 'pointer',
      rawForce: null, tiltX: tilt(event, 'tiltX'), tiltY: tilt(event, 'tiltY'), layerIds: [...layerIds]};
    active.set(event.pointerId, state);
    try { element.setPointerCapture?.(event.pointerId); } catch { /* Capture may be unavailable. */ }
    emit(event, state, 'down');
  });
  listen('pointermove', (event) => {
    const state = active.get(event.pointerId);
    if (!state) return;
    const samples = typeof event.getCoalescedEvents === 'function' ? event.getCoalescedEvents() : [];
    for (const sample of samples.length ? samples : [event]) {
      Object.assign(state, coordinates(sample, state));
      if (state.pointerType !== 'mouse') {
        state.pressure = penPressure(sample);
        state.pressureSource = state.pressure === null ? null : 'pointer';
        state.tiltX = tilt(sample, 'tiltX'); state.tiltY = tilt(sample, 'tiltY');
      }
      emit(sample, state, 'move');
    }
  });
  const finish = (event, phase) => {
    const state = active.get(event.pointerId);
    if (!state) return;
    Object.assign(state, coordinates(event, state));
    if (state.pressure !== null) state.pressure = 0;
    emit(event, state, phase);
    active.delete(event.pointerId);
    try {
      if (element.hasPointerCapture?.(event.pointerId)) element.releasePointerCapture(event.pointerId);
    } catch { /* The browser may already have released capture. */ }
  };
  listen('pointerup', (event) => finish(event, 'up'));
  listen('pointercancel', (event) => finish(event, 'cancel'));
  listen('lostpointercapture', (event) => finish(event, 'cancel'));
  listen('webkitmouseforcewillbegin', (event) => {
    if (preventForceDefault && event.cancelable) event.preventDefault();
  });
  listen('webkitmouseforcechanged', (event) => {
    // A WebKit mouse force event has no pointerId. Only a unique mouse contact can own it.
    const mice = [...active.values()].filter((entry) => entry.pointerType === 'mouse');
    const state = mice.length === 1 ? mice[0] : null;
    const rawForce = event.webkitForce;
    if (!state || !Number.isFinite(rawForce)) return;
    const click = view.MouseEvent?.WEBKIT_FORCE_AT_MOUSE_DOWN;
    const deep = view.MouseEvent?.WEBKIT_FORCE_AT_FORCE_MOUSE_DOWN;
    const pressure = Number.isFinite(click) && Number.isFinite(deep) && deep > click
      ? clamp((rawForce - click) / (deep - click), 0, 1) : null;
    Object.assign(state, coordinates(event, state), {rawForce, pressure, pressureSource: 'webkitForce'});
    emit(event, state, 'move');
  });
  return {dispose() {
    for (const [type, handler] of listeners) element.removeEventListener(type, handler);
    for (const id of active.keys()) {
      try { if (element.hasPointerCapture?.(id)) element.releasePointerCapture(id); } catch { /* Already released. */ }
    }
    active.clear();
  }};
}
