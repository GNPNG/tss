import { LiquidRenderer, LIQUID_PALETTES } from './liquid-renderer.js';
import { installLocalLyrics } from './local-lyrics.js';
import { analyzeAudio, featuresAt } from './audio-features.js';
import { NativeWheelSurface, hasRelease } from './native-wheel.js';
import { NativeWheelExport } from './native-wheel-export.js';
import { VideoFeedbackControls } from './video-feedback.js';
import { installPerformerDeck } from './performer-host.js';
import { ProjectorDeck } from './projector-output.js';
import { initMacroDeck, songFingerprint } from './gesture-macros.js';

const byId = (id) => document.getElementById(id);
const preview = byId('preview');
const context = preview.getContext('2d');
const paletteNames = {
  orchid: 'Magenta, lime and violet', lagoon: 'Blue, cyan and green',
  ember: 'Crimson and gold', twilight: 'Violet, rose and turquoise',
  sunrise: 'Orange and fuchsia', meadow: 'Green and gold',
};
const mathmosWheels = new Map();
const wheelSelect = byId('mathmos-wheel');
const wheelStudy = byId('wheel-study');
const wheelVideo = byId('wheel-study-video');
const windowsDesktop = !!window.__TAURI__?.core && /Windows/i.test(navigator.userAgent);
let startingNative = windowsDesktop;
let projectorFocus = null;
let projectorDeck = null;
let wheelMediaRequest;
const state = {
  audioContext: null, analyser: null, audioBuffer: null, source: null,
  playing: false, startedAt: 0, position: 0, audioName: '', audioPath: null, analysis: null, fileToken: 0, playToken: 0,
  layers: [], selected: null, nextId: 1, renderError: null, exporting: false, exportFinalizing: false, cancelExport: false,
};

function liveSelection() {
  const layer = selectedLayer();
  if (!windowsDesktop || !layer?.visible || !layer.nativeLive || state.exporting ||
      layer.customColors || !mathmosWheels.has(layer.palette)) return null;
  return { layerId: layer.id, wheel: layer.palette, slide: layer.slide, gesture: layer.gesture,
    motion: layer.motion, motionTake: layer.motionTake, visibleFilms: layer.visibleFilms ?? 'both',
    mousePressure: layer.mousePressure?.[layer.slide] ?? 0.5,
    tugResponse: layer.tugResponse[layer.slide], renderWidth: layer.renderWidth, focus: projectorFocus,
    ...feedbackDeck.config(projectorDeck?.enabled), keepPresenting:!!projectorDeck?.enabled };
}
const feedbackDeck = new VideoFeedbackControls(byId('video-feedback'), () => nativeSurface?.sync());
function anchorFeedback(discontinuity = false, ended = false) {
  feedbackDeck.clock({loaded:!!state.audioBuffer, playing:state.playing, position:currentPosition(),ended},discontinuity);
  if(discontinuity) projectorDeck?.flush();
  if(projectorDeck?.enabled) nativeSurface.sync();
}

function reportNativeStats(stats) {
  const selected = liveSelection();
  if (!selected) return;
  const readout = byId('native-frame-rate');
  if (stats.error) {
    readout.textContent = `Native preview unavailable · ${stats.error}`;
    return;
  }
  if (!stats.running || stats.layerId !== selected.layerId || !stats.fps) {
    readout.textContent = 'Waiting for native frame timing…';
    return;
  }
  readout.textContent = `${stats.adapter} · ${stats.width} × ${stats.height} · ${stats.fps.toFixed(1)} FPS · p95 ${stats.p95Ms.toFixed(1)} ms · queue limit ${stats.frameLatency} frames`;
}

const nativeSurface = windowsDesktop ? new NativeWheelSurface(
  window.__TAURI__.core.invoke, preview, liveSelection, reportNativeStats
) : null;
async function prepareWheel(){
  if(!liveSelection()) throw new Error('Select a visible live Mathmos wheel first.');
  preview.scrollIntoView({block:'nearest'});
  await nativeSurface.sync();
}
if (windowsDesktop) projectorDeck = new ProjectorDeck({
  invoke:window.__TAURI__.core.invoke,listen:window.__TAURI__.event.listen,
  prepare:prepareWheel,ensureAudio,onChange:()=>nativeSurface.sync(),
});
const macroDeck = windowsDesktop ? initMacroDeck({
  invoke: window.__TAURI__.core.invoke,
  listen: window.__TAURI__.event.listen,
  clock: () => ({position:currentPosition(),playing:state.playing}),
  selection: liveSelection,
  prepare:prepareWheel,
  seek,
}) : null;

const localLyrics = window.__TAURI__?.core ? installLocalLyrics({
  invoke: window.__TAURI__.core.invoke,
  apply: macroDeck ? groups => macroDeck.applyLyricGroups(groups) : null,
}) : null;

const wheelExporter = windowsDesktop ? new NativeWheelExport({
  invoke: window.__TAURI__.core.invoke, listen: window.__TAURI__.event.listen, selection: liveSelection,
  source: () => ({audioPath: state.audioPath, duration: state.audioBuffer?.duration, proceduralBusy: state.exporting}),
  getEvents: (id, duration) => macroDeck?.exportEvents(id, duration) ?? [],
  finishRecording: async () => { await macroDeck?.finishPendingRecording(); },
  pause, prepare: prepareWheel, sync: () => nativeSurface.sync(), status,
}) : null;

const performerDeck = windowsDesktop ? installPerformerDeck({
  invoke:window.__TAURI__.core.invoke, listen:window.__TAURI__.event.listen,
  selection:liveSelection, prepare:prepareWheel,
  layers:()=>state.layers.filter(l=>l.nativeLive && !l.customColors && mathmosWheels.has(l.palette)),
}) : null;

function status(message) { byId('app-status').textContent = message; }
function formatTime(seconds) {
  const total = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`;
}
function currentPosition() {
  if (!state.audioBuffer) return 0;
  if (!state.playing) return state.position;
  return Math.min(state.audioBuffer.duration, state.position + state.audioContext.currentTime - state.startedAt);
}
function updateTransport() {
  localLyrics?.source(state.audioPath, state.audioBuffer);
  const ready = !!state.audioBuffer;
  byId('play').disabled = !ready;
  byId('restart').disabled = !ready;
  byId('stop').disabled = !ready;
  byId('export').disabled = !state.exporting && (!ready || !state.audioPath || !window.__TAURI__?.core);
  wheelExporter?.refresh();
  byId('play').textContent = state.playing ? '❚❚' : '▶';
  byId('play').setAttribute('aria-label', state.playing ? 'Pause' : 'Play');
  byId('play').title = state.playing ? 'Pause' : 'Play';
  byId('current-time').textContent = formatTime(currentPosition());
  byId('total-time').textContent = formatTime(state.audioBuffer?.duration || 0);
  byId('playhead').style.left = `${state.audioBuffer ? currentPosition() / state.audioBuffer.duration * 100 : 0}%`;
}
function ensureAudio() {
  if (!state.audioContext) {
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) throw new Error('This browser does not support audio playback.');
    state.audioContext = new AudioContextClass();
    state.analyser = state.audioContext.createAnalyser();
    state.analyser.fftSize = 2048;
    state.analyser.smoothingTimeConstant = 0.8;
    if(projectorDeck) projectorDeck.attach(state.audioContext,state.analyser);
    else state.analyser.connect(state.audioContext.destination);
  }
  return state.audioContext;
}
function stopSource() {
  if (state.source) {
    const old = state.source;
    state.source = null;
    old.onended = null;
    old.stop();
    old.disconnect();
  }
}
async function play() {
  if (!state.audioBuffer || state.playing) return;
  const token = ++state.playToken;
  const audio = ensureAudio();
  await audio.resume();
  if (token !== state.playToken || !state.audioBuffer || state.playing) return;
  if (state.position >= state.audioBuffer.duration) state.position = 0;
  const source = audio.createBufferSource();
  source.buffer = state.audioBuffer;
  source.connect(state.analyser);
  source.onended = () => {
    if (state.source !== source) return;
    state.source = null;
    state.position = state.audioBuffer.duration;
    state.playing = false;
    anchorFeedback(false,true);
    macroDeck?.transportChanged();
    updateTransport();
    status('Playback finished');
  };
  state.source = source;
  state.startedAt = audio.currentTime;
  state.playing = true;
  source.start(0, state.position);
  anchorFeedback();
  macroDeck?.transportChanged(state.position);
  updateTransport();
  status('Playing');
}
function pause() {
  state.playToken++;
  if (!state.playing) return;
  state.position = currentPosition();
  state.playing = false;
  anchorFeedback(true);
  macroDeck?.transportChanged();
  stopSource();
  updateTransport();
  status('Paused');
}
async function seek(position) {
  if (!state.audioBuffer) return;
  state.playToken++;
  const wasPlaying = state.playing;
  if (wasPlaying) pause();
  state.position = Math.max(0, Math.min(state.audioBuffer.duration, position));
  anchorFeedback(true);
  macroDeck?.transportChanged();
  updateTransport();
  if (wasPlaying && state.position < state.audioBuffer.duration) await play();
}
async function loadAudio(file, beforeCommit = async () => {}) {
  if (!file) return;
  const token = ++state.fileToken;
  status('Reading audio…');
  try {
    const audio = ensureAudio();
    const bytes = await file.arrayBuffer();
    const fingerprint = macroDeck ? songFingerprint(bytes).catch(() => null) : null;
    const decoded = await audio.decodeAudioData(bytes);
    if (token !== state.fileToken) return;
    status('Analyzing audio…');
    const analysis = await analyzeAudio(decoded, (progress) => status(`Analyzing audio ${Math.round(progress * 100)}%`), () => token === state.fileToken);
    const songId = await fingerprint;
    if (!analysis || token !== state.fileToken) return false;
    await beforeCommit();
    if (token !== state.fileToken) return false;
    pause();
    state.audioBuffer = decoded;
    state.analysis = analysis;
    state.audioName = file.name;
    state.position = 0;
    anchorFeedback(true);
    if (macroDeck) {
      try { await macroDeck.songLoaded(songId,analysis,() => token === state.fileToken); }
      catch(error) { macroDeck.reportError(error); }
      if (token !== state.fileToken) return false;
    }
    byId('track-title').textContent = file.name;
    byId('source-name').textContent = file.name;
    byId('source-meta').textContent = `${formatTime(decoded.duration)} · ${Math.round(decoded.sampleRate / 1000)} kHz · ${decoded.numberOfChannels === 1 ? 'Mono' : 'Stereo'}`;
    byId('empty-wave').hidden = true;
    updateTransport();
    drawWaveform();
    renderTimeline();
    status('Audio loaded');
    return true;
  } catch (error) {
    if (token !== state.fileToken) return;
    status(`Could not open audio: ${error.message}`);
    return false;
  }
}
function drawWaveform() {
  const canvas = byId('waveform');
  const rect = canvas.getBoundingClientRect();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.max(1, Math.round(rect.width * dpr));
  canvas.height = Math.max(1, Math.round(rect.height * dpr));
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (!state.audioBuffer) return;
  const channel = state.audioBuffer.getChannelData(0);
  const step = Math.max(1, Math.floor(channel.length / canvas.width));
  ctx.fillStyle = '#b6b98a';
  const mid = canvas.height / 2;
  for (let x = 0; x < canvas.width; x++) {
    let peak = 0;
    const start = Math.floor(x * channel.length / canvas.width);
    for (let i = start; i < Math.min(start + step, channel.length); i += Math.max(1, Math.floor(step / 40))) {
      peak = Math.max(peak, Math.abs(channel[i]));
    }
    const height = Math.max(dpr, peak * canvas.height * 0.9);
    ctx.fillRect(x, mid - height / 2, 1, height);
  }
}
function rotateColor(hex, degrees) {
  const values = hex.slice(1).match(/../g).map((part) => parseInt(part, 16) / 255);
  const max = Math.max(...values), min = Math.min(...values), delta = max - min;
  let hue = 0;
  if (delta) {
    if (max === values[0]) hue = ((values[1] - values[2]) / delta) % 6;
    else if (max === values[1]) hue = (values[2] - values[0]) / delta + 2;
    else hue = (values[0] - values[1]) / delta + 4;
  }
  hue = ((hue * 60 + degrees) % 360 + 360) % 360;
  const chroma = delta, x = chroma * (1 - Math.abs((hue / 60) % 2 - 1)), m = min;
  let rgb;
  if (hue < 60) rgb = [chroma, x, 0];
  else if (hue < 120) rgb = [x, chroma, 0];
  else if (hue < 180) rgb = [0, chroma, x];
  else if (hue < 240) rgb = [0, x, chroma];
  else if (hue < 300) rgb = [x, 0, chroma];
  else rgb = [chroma, 0, x];
  return rgb.map((component) => component + m);
}
function selectedLayer() { return state.layers.find((layer) => layer.id === state.selected); }
function applyPalette(layer, name, colors) {
  layer.palette = name;
  layer.colors = [...colors];
  layer.customColors = false;
  renderControls();
}

async function loadWheelPresets() {
  try {
    const response = await fetch(new URL('./assets/mathmos-wheels.json', import.meta.url));
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const { version, wheels } = await response.json();
    if (version !== 1 || !Array.isArray(wheels) || wheels.length === 0) throw new Error('Invalid wheel list');
    const presets = wheels.map(({ id, name, swatches }) => {
      const colors = [swatches.carrier, swatches.rear, swatches.overlap, swatches.front];
      if (!/^[a-z]+(?:-[a-z]+)+$/.test(id) || !name || colors.some((color) => !/^#[0-9a-f]{6}$/i.test(color))) {
        throw new Error('Invalid wheel preset');
      }
      return { id, name, colors };
    });
    for (const preset of presets) {
      mathmosWheels.set(preset.id, preset);
      wheelSelect.add(new Option(preset.name, preset.id));
    }
    if (windowsDesktop) {
      const benchmark = await window.__TAURI__.core.invoke('live_wheel_benchmark_config');
      if (benchmark) {
        await window.__TAURI__.event.listen('tss-benchmark-release', () => byId('replay-release').click());
        await window.__TAURI__.event.listen('tss-benchmark-wheel', ({ payload }) => {
          for (const [id, value] of [['wheel-slide', payload.slide], ['wheel-gesture', payload.gesture]]) {
            const control = byId(id);
            control.value = value;
            control.dispatchEvent(new Event('change'));
          }
          chooseWheel(payload.wheel);
        });
      }
      // Both paths select a wheel through the same editor control.
      const layer = selectedLayer();
      if (layer && (benchmark || (layer.id === 1 && layer.palette === 'orchid' && !layer.customColors))) {
        layer.slide = benchmark?.slide ?? 'rear';
        layer.gesture = benchmark?.gesture ?? 'tug';
        layer.renderWidth = benchmark?.renderWidth ?? 640;
        chooseWheel(benchmark?.wheel ?? 'violet-blue');
        if (benchmark) {
          const control = byId('wheel-motion');
          control.value = benchmark.motion;
          control.dispatchEvent(new Event('change'));
          if (benchmark.focus != null) {
            const focus = byId('wheel-focus');
            focus.value = String(Math.round(100 * benchmark.focus));
            focus.dispatchEvent(new Event('input'));
          }
        }
        if (benchmark?.verifyExport) {
          const { verifyWheelExport } = await import('./verify-wheel-export.js');
          await nativeSurface.sync();
          await verifyWheelExport(liveSelection(), () => state.audioBuffer?.duration, status, projectorDeck,performerDeck);
        }
      }
    }
  } catch (error) {
    wheelSelect.options[0].textContent = 'Wheel presets unavailable';
    status(`Could not read oil wheels: ${error.message}`);
  } finally {
    startingNative = false;
    renderControls();
  }
}

function chooseWheel(id) {
  if (!mathmosWheels.has(id)) return;
  wheelSelect.value = id;
  wheelSelect.dispatchEvent(new Event('change'));
}

function makeLayer() {
  const canvas = document.createElement('canvas');
  canvas.width = preview.width;
  canvas.height = preview.height;
  const id = state.nextId++;
  return { id, name: `Liquid ${id}`, visible: true, palette: 'orchid', colors: [...LIQUID_PALETTES.orchid], customColors: false, hue: 0, speed: 1, scale: 1, opacity: state.layers.length ? 0.7 : 1, canvas, renderer: null, nativeLive: windowsDesktop,
    motion: 'wheel-flow', motionTake: 0, visibleFilms: 'both',
    slide: 'rear', gesture: 'tug', tugResponse: { rear: 0.4, front: 0.4 }, mousePressure: { rear: 0.5, front: 0.5 }, renderWidth: 640 };
}
function selectLayer(id) {
  state.selected = id;
  renderTimeline();
  renderControls();
}
function addLayer() {
  try {
    const layer = makeLayer();
    state.layers.unshift(layer);
    selectLayer(layer.id);
    status(`${layer.name} added`);
  } catch (error) { showRenderError(error); }
}
function showRenderError(error) {
  state.renderError = error;
  byId('render-error').hidden = false;
  byId('render-error').textContent = `Liquid preview unavailable: ${error.message}`;
  status('Liquid preview unavailable');
}
function layerButton(text, label, action, disabled = false) {
  const button = document.createElement('button');
  button.type = 'button'; button.className = 'layer-control'; button.textContent = text;
  button.title = label; button.setAttribute('aria-label', label); button.disabled = disabled;
  button.addEventListener('click', (event) => { event.stopPropagation(); action(); });
  return button;
}
function renderTimeline() {
  const list = byId('layer-list'), lanes = byId('visual-lanes');
  list.replaceChildren(); lanes.replaceChildren();
  state.layers.forEach((layer, index) => {
    const row = document.createElement('div');
    row.className = `layer-row${layer.id === state.selected ? ' selected' : ''}`;
    row.tabIndex = 0; row.setAttribute('role', 'button'); row.setAttribute('aria-label', `Select ${layer.name}`);
    row.addEventListener('click', () => selectLayer(layer.id));
    row.addEventListener('keydown', (event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectLayer(layer.id); } });
    const icon = document.createElement('span'); icon.className = 'layer-icon'; icon.textContent = '◉';
    const info = document.createElement('div'); info.className = 'layer-text';
    const title = document.createElement('strong'); title.textContent = layer.name;
    const subtitle = document.createElement('small'); subtitle.textContent = layer.visible ? 'Liquid light' : 'Hidden';
    info.append(title, subtitle);
    const toggle = layerButton(layer.visible ? '●' : '○', `${layer.visible ? 'Hide' : 'Show'} ${layer.name}`, () => { layer.visible = !layer.visible; renderTimeline(); renderControls(); });
    const up = layerButton('↑', `Move ${layer.name} up`, () => { [state.layers[index - 1], state.layers[index]] = [state.layers[index], state.layers[index - 1]]; renderTimeline(); }, index === 0);
    const down = layerButton('↓', `Move ${layer.name} down`, () => { [state.layers[index + 1], state.layers[index]] = [state.layers[index], state.layers[index + 1]]; renderTimeline(); }, index === state.layers.length - 1);
    const remove = layerButton('×', `Remove ${layer.name}`, () => {
      layer.renderer?.dispose(); state.layers.splice(index, 1);
      state.renderError = null;
      byId('render-error').hidden = true;
      if (state.selected === layer.id) state.selected = state.layers[0]?.id ?? null;
      renderTimeline(); renderControls(); status(`${layer.name} removed`);
      if (nativeSurface) nativeSurface.sync().then(() =>
        window.__TAURI__.core.invoke('forget_live_wheel_layer', { layerId: layer.id })
      ).catch((e) => status(`Could not release layer: ${e}`));
    });
    row.append(icon, info, toggle, up, down, remove); list.append(row);
    const lane = document.createElement('div'); lane.className = 'visual-lane';
    const clip = document.createElement('div'); clip.className = `visual-clip${layer.visible ? '' : ' hidden'}`;
    const clipName = document.createElement('span'); clipName.textContent = layer.name;
    const clipDuration = document.createElement('small'); clipDuration.textContent = state.audioBuffer ? formatTime(state.audioBuffer.duration) : 'Full track';
    clip.append(clipName, clipDuration); lane.append(clip); lanes.append(lane);
  });
  byId('layer-count').textContent = `${state.layers.length} visual ${state.layers.length === 1 ? 'layer' : 'layers'}`;
  const ruler = byId('ruler'); ruler.replaceChildren();
  for (let i = 0; i <= 8; i++) {
    const tick = document.createElement('span'); tick.textContent = state.audioBuffer ? formatTime(state.audioBuffer.duration * i / 8) : '--:--'; ruler.append(tick);
  }
}
function renderControls() {
  const layer = selectedLayer();
  byId('selected-name').textContent = layer?.name || 'No layer selected';
  const grid = byId('palette-grid'); grid.replaceChildren();
  Object.entries(LIQUID_PALETTES).forEach(([name, colors]) => {
    const button = document.createElement('button'); button.className = `palette${layer?.palette === name && !layer?.customColors ? ' active' : ''}`;
    button.title = paletteNames[name] || name; button.setAttribute('aria-label', paletteNames[name] || name);
    button.setAttribute('aria-pressed', String(layer?.palette === name && !layer?.customColors)); button.disabled = !layer;
    colors.forEach((color) => { const swatch = document.createElement('span'); swatch.style.backgroundColor = color; button.append(swatch); });
    button.addEventListener('click', () => applyPalette(layer, name, colors)); grid.append(button);
  });
  const wheel = layer && mathmosWheels.get(layer.palette);
  const live = liveSelection();
  wheelExporter?.refresh();
  byId('video-feedback').hidden = !live;
  byId('native-wheel-controls').hidden = !windowsDesktop || !wheel;
  byId('native-performance').hidden = !live;
  byId('live-wheel').checked = !!live;
  byId('live-wheel').disabled = !wheel || !!layer?.customColors || !layer?.visible;
  byId('wheel-motion').value = layer?.motion ?? 'wheel-flow';
  byId('wheel-motion').disabled = !live;
  byId('wheel-films').value = layer?.visibleFilms ?? 'both';
  byId('wheel-films').disabled = !live;
  byId('replay-release').hidden = !hasRelease(layer?.motion);
  byId('replay-release').textContent = layer?.motion === 'wheel-with-coral-beads'
    ? 'Replay front beads' : 'Replay release live';
  byId('replay-release').disabled = !live;
  byId('wheel-slide').value = layer?.slide ?? 'rear';
  byId('wheel-width').value = String(layer?.renderWidth ?? 640);
  byId('wheel-gesture').value = layer?.gesture ?? 'tug';
  byId('wheel-gesture').disabled = byId('wheel-slide').disabled = byId('wheel-width').disabled = !live;
  byId('tug-response').value = String(layer?.tugResponse[layer.slide] ?? 0.4);
  byId('tug-response').disabled = !live || layer.gesture !== 'tug';
  byId('mouse-pressure-controls').hidden = !live || layer.gesture !== 'pressure';
  const strength = Math.round(100 * (layer?.mousePressure?.[layer?.slide] ?? .5));
  byId('mouse-pressure').value = strength;
  byId('mouse-pressure-value').textContent = strength+'%';
  byId('wheel-focus').disabled = !live;
  updateFocusControl();
  preview.parentElement.classList.toggle('native-active', !!live);
  byId('preview-status').textContent = live ? 'NATIVE GPU · SELECTED WHEEL' : 'PROCEDURAL PREVIEW';
  byId('native-instruction').textContent = layer?.gesture === 'rotate'
    ? `Drag around the center to turn the ${layer.slide} glass slide.`
    : layer?.gesture === 'pressure' ? `Press and hold the ${layer.slide} slide to compress it. Drag the contact point. Release to restore the gap.`
    : `Drag to tug the material in the ${layer?.slide ?? 'rear'} slide.`;
  byId('audio-note').hidden = !!live;
  for (const id of ['hue', 'speed', 'scale', 'opacity']) byId(id).disabled = !layer || !!live;
  byId('export').title = 'Export the procedural timeline composite';
  if (!state.exporting) byId('export').textContent = live ? 'Export procedural preview' : 'Export 4K preview';
  if (live && !nativeSurface.surfaceRequested) byId('native-frame-rate').textContent = 'Starting native wheel…';
  nativeSurface?.sync();
  wheelSelect.disabled = !layer || mathmosWheels.size === 0;
  wheelSelect.value = wheel && !layer.customColors ? wheel.id : '';
  if (mathmosWheels.size) wheelSelect.options[0].textContent = wheel && layer.customColors ? `Custom · ${wheel.name}` : 'Choose a wheel';
  byId('show-wheel-study').disabled = !wheel;
  byId('pigment-heading').textContent = wheel ? 'PREVIEW SWATCHES' : 'COLOR LAYERS';
  const pigments = byId('pigment-colors'); pigments.replaceChildren();
  if (layer) layer.colors.forEach((color, index) => {
    const label = document.createElement('label');
    const name = wheel ? ['Clear', 'Rear slide', 'Overlap', 'Front slide'][index] : `Color ${index + 1}`;
    label.textContent = name;
    const picker = document.createElement('input');
    picker.type = 'color'; picker.value = color;
    picker.disabled = !!live;
    picker.setAttribute('aria-label', wheel ? `${name} preview color` : `Liquid color ${index + 1}`);
    picker.addEventListener('input', () => { layer.colors[index] = picker.value; layer.customColors = true; });
    picker.addEventListener('change', renderControls);
    label.append(picker); pigments.append(label);
  });
  if (!layer) return;
  ['hue', 'speed', 'scale', 'opacity'].forEach((id) => { byId(id).value = layer[id]; });
  byId('hue-label').textContent = `${layer.hue}°`;
  byId('speed-output').textContent = `${layer.speed.toFixed(1)}×`;
  byId('scale-output').textContent = `${layer.scale.toFixed(1)}×`;
  byId('opacity-output').textContent = `${Math.round(layer.opacity * 100)}%`;
}
function renderFrame(timestamp) {
  macroDeck?.tick();
  if (context && !startingNative && !liveSelection() && !nativeSurface?.surfaceRequested &&
      !state.renderError && !state.exporting && !document.hidden && !document.querySelector('dialog[open]')) {
    try {
      context.clearRect(0, 0, preview.width, preview.height);
      const time = state.audioBuffer ? currentPosition() : timestamp / 1000;
      const features = featuresAt(state.analysis, time);
      for (const layer of [...state.layers].reverse()) {
        if (!layer.visible) continue;
        if (!layer.renderer) {
          layer.renderer = new LiquidRenderer(layer.canvas);
          layer.renderer.resize(layer.canvas.width, layer.canvas.height);
        }
        const palette = layer.colors.map((color) => rotateColor(color, layer.hue));
        layer.renderer.render({time, ...features, palette, speed: layer.speed, scale: layer.scale});
        context.globalAlpha = layer.opacity;
        context.drawImage(layer.canvas, 0, 0);
      }
      context.globalAlpha = 1;
    } catch (error) { showRenderError(error); }
  }
  if (state.playing) updateTransport();
  requestAnimationFrame(renderFrame);
}
window.tssLoadAudioBytes = (name, bytes, beforeCommit) => loadAudio(new File([bytes instanceof ArrayBuffer || ArrayBuffer.isView(bytes) ? bytes : new Uint8Array(bytes)], name), beforeCommit);
if (window.__TAURI__?.core) {
  window.tssImportAudio = async (grantedSelection) => {
    const invoke = window.__TAURI__.core.invoke;
    byId('open-audio').disabled = true;
    try {
      const selection = Array.isArray(grantedSelection) ? grantedSelection : await invoke('choose_audio');
      if (!selection) return;
      const [selectionId, path] = selection;
      const bytes = await invoke('read_pending_audio', { selectionId });
      const name = path.split(/[\\/]/).pop();
      if (await window.tssLoadAudioBytes(name, bytes, () => invoke('confirm_audio', { selectionId }))) {
        state.audioPath = path;
        updateTransport();
        if (macroDeck) {
          const token = state.fileToken;
          try {
            const sidecar = await invoke('read_timed_lyrics',{selectionId});
            if (token === state.fileToken) macroDeck.importSidecar(sidecar);
          } catch(error) { if(token === state.fileToken) macroDeck.reportError(error); }
        }
      }
    } finally {
      byId('open-audio').disabled = false;
    }
  };
}

async function exportVideo() {
  if (state.exporting) {
    state.cancelExport = true;
    status('Cancelling export…');
    if (state.exportFinalizing) await window.__TAURI__.core.invoke('cancel_export');
    return;
  }
  if (!state.audioPath || !state.audioBuffer || !window.__TAURI__?.core) return;
  const invoke = window.__TAURI__.core.invoke;
  const outputPath = await invoke('choose_export_path');
  if (!outputPath) return;
  const audioPath = state.audioPath;
  const audioDuration = state.audioBuffer.duration;
  const analysis = state.analysis;
  const layers = state.layers.filter((layer) => layer.visible).map((layer) => ({
    colors: [...layer.colors], hue: layer.hue, speed: layer.speed, scale: layer.scale, opacity: layer.opacity,
  }));
  const width = 3840, height = 2160, fps = 30;
  const frameCount = Math.ceil(audioDuration * fps);
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const output = canvas.getContext('2d');
  if (!output) throw new Error('Unable to create the export canvas');
  const renderers = [];
  let started = false;
  pause();
  state.exporting = true;
  state.cancelExport = false;
  byId('export').textContent = 'Cancel export';
  updateTransport();
  try {
    await nativeSurface?.sync();
    for (const layer of layers) {
      const layerCanvas = document.createElement('canvas');
      const renderer = new LiquidRenderer(layerCanvas);
      renderer.resize(width, height);
      if (renderer.gl.drawingBufferWidth !== width || renderer.gl.drawingBufferHeight !== height) {
        renderer.dispose();
        throw new Error('The graphics device could not allocate a 4K render buffer');
      }
      renderers.push({ ...layer, canvas: layerCanvas, renderer });
    }
    await invoke('start_export', { audioPath, outputPath, width, height, fps });
    started = true;
    for (let index = 0; index < frameCount; index++) {
      if (state.cancelExport) break;
      const time = index / fps;
      output.fillStyle = '#000';
      output.globalAlpha = 1;
      output.fillRect(0, 0, width, height);
      for (const layer of [...renderers].reverse()) {
        const palette = layer.colors.map((color) => rotateColor(color, layer.hue));
        layer.renderer.render({ time, ...featuresAt(analysis, time), palette, speed: layer.speed, scale: layer.scale });
        output.globalAlpha = layer.opacity;
        output.drawImage(layer.canvas, 0, 0);
      }
      output.globalAlpha = 1;
      const pngBase64 = canvas.toDataURL('image/png');
      if (!pngBase64.startsWith('data:image/png;base64,')) throw new Error('Could not encode the rendered frame');
      await invoke('append_export_frame', { index, pngBase64 });
      status(`Exporting 4K ${Math.round((index + 1) * 100 / frameCount)}%`);
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    if (state.cancelExport) {
      await invoke('cancel_export');
      started = false;
      status('Export cancelled');
    } else {
      state.exportFinalizing = true;
      byId('export').textContent = 'Cancel export';
      status('Finishing export…');
      updateTransport();
      await invoke('finish_export');
      started = false;
      status(`Export saved to ${outputPath}`);
    }
  } catch (error) {
    if (started) await invoke('cancel_export').catch(() => {});
    status(state.cancelExport ? 'Export cancelled' : `Export failed: ${error.message || error}`);
  } finally {
    renderers.forEach(({ renderer }) => renderer.dispose());
    state.exporting = false;
    state.exportFinalizing = false;
    state.cancelExport = false;
    byId('export').textContent = 'Export 4K preview';
    updateTransport(); renderControls();
  }
}
byId('open-audio').addEventListener('click', () => {
  if (typeof window.tssImportAudio === 'function') {
    Promise.resolve(window.tssImportAudio()).catch((error) => status(`Could not open audio: ${error.message || error}`));
  } else {
    byId('audio-file').click();
  }
});
byId('audio-file').addEventListener('change', (event) => { state.audioPath = null; loadAudio(event.target.files?.[0]); event.target.value = ''; });
byId('export').addEventListener('click', () => exportVideo().catch((error) => status(`Export failed: ${error.message || error}`)));
byId('show-logo').addEventListener('click', () => { byId('logo-dialog').showModal(); nativeSurface?.sync(); });
byId('close-logo').addEventListener('click', () => byId('logo-dialog').close());
byId('logo-dialog').addEventListener('close', () => nativeSurface?.sync());
byId('live-wheel').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.nativeLive = event.target.checked; renderControls(); }
});
byId('wheel-slide').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.slide = event.target.value; renderControls(); }
});
byId('wheel-films').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.visibleFilms = event.target.value; nativeSurface?.sync(); }
});
byId('wheel-motion').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (!layer || layer.motion === event.target.value) return;
  layer.motion = event.target.value;
  if (hasRelease(layer.motion)) layer.motionTake++;
  renderControls();
});
byId('replay-release').addEventListener('click', () => {
  const layer = selectedLayer();
  if (!layer || !hasRelease(layer.motion)) return;
  layer.motionTake++;
  nativeSurface?.sync();
});
byId('wheel-gesture').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.gesture = event.target.value; renderControls(); }
});
byId('tug-response').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.tugResponse[layer.slide] = Number(event.target.value); renderControls(); }
});
byId('mouse-pressure').addEventListener('input', event => {
  const layer = selectedLayer();
  if (!layer) return;
  const value = Number(event.target.value);
  layer.mousePressure ??= {rear:.5, front:.5};
  layer.mousePressure[layer.slide] = value / 100;
  byId('mouse-pressure-value').textContent = value+'%';
  nativeSurface?.sync();
});
byId('wheel-width').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer) { layer.renderWidth = Number(event.target.value); renderControls(); }
});
function updateFocusControl() {
  const value = Math.round(100 * (projectorFocus ?? 0.5));
  const label = projectorFocus === null ? 'Wheel setting'
    : value === 0 ? 'Rear slide' : value === 100 ? 'Front slide' : `${value}% toward front`;
  byId('wheel-focus').value = value;
  byId('wheel-focus').setAttribute('aria-valuetext', label);
  byId('wheel-focus-value').textContent = label;
  byId('wheel-focus-reset').disabled = projectorFocus === null || !liveSelection();
}
byId('wheel-focus').addEventListener('input', (event) => {
  projectorFocus = Number(event.target.value) / 100;
  updateFocusControl(); nativeSurface?.sync();
});
byId('wheel-focus-reset').addEventListener('click', () => {
  projectorFocus = null;
  updateFocusControl(); nativeSurface?.sync();
});
wheelSelect.addEventListener('change', () => {
  const layer = selectedLayer();
  const preset = mathmosWheels.get(wheelSelect.value);
  if (layer && preset) { layer.nativeLive = windowsDesktop; applyPalette(layer, preset.id, preset.colors); }
});
byId('show-wheel-study').addEventListener('click', async () => {
  const preset = mathmosWheels.get(selectedLayer()?.palette);
  if (!preset) return;
  byId('wheel-study-name').textContent = preset.name;
  byId('wheel-study-error').hidden = true;
  wheelStudy.showModal();
  nativeSurface?.sync();
  const request = new AbortController();
  wheelMediaRequest = request;
  try {
    // Tauri returns bundled assets as complete responses. Give the player a
    // seekable local blob so playback works independently of HTTP range support.
    const path = new URL(`./assets/wheel-previews/${preset.id}.mp4`, import.meta.url);
    const response = await fetch(path, { signal: request.signal });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const clip = await response.blob();
    if (request.signal.aborted || !wheelStudy.open) return;
    wheelVideo.src = URL.createObjectURL(clip);
    // Controls remain available when the platform requires another play gesture.
    wheelVideo.play().catch(() => {});
  } catch {
    if (!request.signal.aborted) byId('wheel-study-error').hidden = false;
  }
});
wheelVideo.addEventListener('error', () => {
  if (wheelStudy.open) byId('wheel-study-error').hidden = false;
});
byId('close-wheel-study').addEventListener('click', () => wheelStudy.close());
wheelStudy.addEventListener('close', () => {
  nativeSurface?.sync();
  wheelMediaRequest?.abort();
  wheelVideo.pause();
  const url = wheelVideo.getAttribute('src');
  wheelVideo.removeAttribute('src');
  wheelVideo.load();
  if (url) URL.revokeObjectURL(url);
});
byId('play').addEventListener('click', () => { if (state.playing) pause(); else play().catch((error) => status(`Playback failed: ${error.message}`)); });
byId('restart').addEventListener('click', () => seek(0).catch((error) => status(error.message)));
byId('stop').addEventListener('click', () => { pause(); state.position = 0; updateTransport(); status('Stopped'); });
byId('add-layer').addEventListener('click', addLayer);
byId('seek-area').addEventListener('click', (event) => { const rect = event.currentTarget.getBoundingClientRect(); seek((event.clientX - rect.left) / rect.width * (state.audioBuffer?.duration || 0)).catch((error) => status(error.message)); });
for (const id of ['hue', 'speed', 'scale', 'opacity']) {
  byId(id).addEventListener('input', (event) => { const layer = selectedLayer(); if (!layer) return; layer[id] = Number(event.target.value); renderControls(); });
}
window.addEventListener('resize', drawWaveform);
window.addEventListener('beforeunload', () => { state.layers.forEach((layer) => layer.renderer?.dispose()); });
if (!context) showRenderError(new Error('Canvas 2D is not available'));
else addLayer();
renderTimeline(); renderControls(); updateTransport(); requestAnimationFrame(renderFrame);
loadWheelPresets();
