import { LiquidRenderer, LIQUID_PALETTES } from './liquid-renderer.js';
import {isNativePalette, paletteFields, rotateColor} from './film-palette.js';
import { installVisualAdvisor } from './visual-advisor.js';
import { installMediaTools } from './media-tools.js';
import { installLocalLyrics } from './local-lyrics.js';
import { analyzeAudio, featuresAt } from './audio-features.js';
import { materialMotionFields, simulationFields, detailScaleFields } from './material-motion.js';
import { NativeWheelSurface, hasRelease } from './native-wheel.js';
import { PlateFormatControls } from './plate-format.js';
import { NativeWheelPlates } from './native-wheel-plates.js';
import { installVisualMedia, newMedia, programLayers } from './visual-media.js';
import { NativeWheelExport } from './native-wheel-export.js';
import { VideoFeedbackControls } from './video-feedback.js';
import { installLegends } from './legends.js';
import { installProjectionLight, effectiveLighting } from './projection-light.js';
import { installPerformerDeck } from './performer-host.js';
import { ProjectorDeck } from './projector-output.js';
import { initMacroDeck, songFingerprint, songGestureEvents } from './gesture-macros.js';
import { PROJECT_FORMAT, layerSettings, validateProject, emptyGestures, EditHistory } from './project-file.js';
import { audioCueGroups } from './phrase-cues.js';
import { advisorIntervals, defaultBindings, glyphs, opacityAt, planIntervals, replaceRegion, validRange } from './timeline.js';
import { transferRegion, removeRegionGestures } from './region-tracks.js';
import {bindProjectMedia} from './project-media.js';

const byId = (id) => document.getElementById(id);
const preview = byId('preview');
let proceduralPreview = null;
const paletteNames = {
  orchid: 'Magenta, lime and violet', lagoon: 'Blue, cyan and green',
  ember: 'Crimson and gold', twilight: 'Violet, rose and turquoise',
  sunrise: 'Orange and fuchsia', meadow: 'Green and gold',
};
const slidePresets = new Map();
const isWheel = layer => isNativePalette(layer, slidePresets);
const slideSelect = byId('slide-preset');
const windowsDesktop = !!window.__TAURI__?.core && /Windows/i.test(navigator.userAgent);
const nativeDesktop = windowsDesktop || (!!window.__TAURI__?.core && /Macintosh/i.test(navigator.userAgent));
let startingNative = nativeDesktop;
let solverAvailable = false;
let recordedBenchmark = false;
const newFluidSeed = () => crypto.getRandomValues(new Uint32Array(1))[0];
const liveFluidTrack = layer => layer?.simulationSeed != null && !layer.media;
const solvingProgram = () => state.layers.some(layer => liveFluidTrack(layer) &&
  (layer.visible || state.layers.some(({visible, media}) => visible && media?.assetId != null &&
    (media.maskA === layer.id || media.maskB === layer.id))));
const missingSolver = () => !solverAvailable && solvingProgram();
let projectorFocus = null;
let projectorDeck = null;
let visualMedia = null;
let projectName = '', detachedGestures = emptyGestures(), savedEdits = '', projectBusy = false;
const editHistory = new EditHistory();
const retainedNativeLayers = new Set();
const visualGrants=new Map();
let highestId = 0;
const state = {
  range:null, savedDuration:0, bindings:defaultBindings(),
  audioContext: null, analyser: null, audioBuffer: null, source: null,
  playing: false, startedAt: 0, position: 0, audioName: '', audioPath: null, analysis: null, fileToken: 0, playToken: 0,
  audioRef: null, audioSelectionId: null,
  layers: [], selected: null, nextId: 1, renderError: null, exporting: false, exportFinalizing: false, cancelExport: false,
};

const lighting=installProjectionLight({selected:selectedLayer, changed:layout=>{
  legends.stop();
  if(layout) renderTimeline();
  else for(const el of byId('layer-list').querySelectorAll('[data-projector]')) el.textContent=lighting.groupLabel(el.dataset.projector);
  updateTransport();nativeSurface?.sync();
}});
const legends=installLegends({
  layers:()=>state.layers,
  preview:()=>{nativeSurface?.sync();updateTransport();},
  apply:(rig,artist)=>{
    for(const [id,beam] of rig.routes) {
      const layer=state.layers.find(l=>l.id===id);
      if(layer)layer.projector=beam;
    }
    projectorFocus=rig.focus;
    lighting.restore(rig.fields);renderTimeline();renderControls();updateTransport();
    checkpoint('Apply '+artist+' rig');
    status('Applied '+artist+' rig to the current project.');
  }
});
byId('legends').hidden=!nativeDesktop;
const usesLiveProjection=()=>!!legends.audition || lighting.usesLiveOptics || state.layers.some(l=>l.projector==='b');
function wheelGestureSelection() { return selectedLayer()?.media ? null : liveSelection(); }
function liveSelection() {
  const layer = selectedLayer();
  if (!nativeDesktop || (startingNative && !slidePresets.size) || !layer || (!isWheel(layer) && !layer.media) || state.exporting || missingSolver()) return null;
  const rig=legends.audition;
  const stack = programLayers(state.layers,isWheel);
  if(rig)for(const item of stack)if(rig.routes.has(item.layerId))item.projector=rig.routes.get(item.layerId);
  if (!stack.some(l=>!l.matteOnly)) return null;
  const timed = stack.some(l=>l.envelope!=null || l.media);
  const needsStack = timed || usesLiveProjection() || stack.length > 1 || stack[0].opacity !== 1 || stack[0].layerId !== layer.id;
  return { ...effectiveLighting(rig?.fields??lighting.fields()), ...(needsStack ? {stack} : {}), layerId: layer.id, wheel: layer.media?'violet-blue':layer.palette, slide: layer.slide, gesture: layer.gesture,
    ...paletteFields(layer), ...simulationFields(layer), ...detailScaleFields(layer), motion: layer.motion, motionTake: layer.motionTake, ...materialMotionFields(layer.media ? null : layer), visibleFilms: layer.visibleFilms ?? 'both',
    mousePressure: layer.mousePressure?.[layer.slide] ?? 0.5,
    tugResponse: layer.tugResponse[layer.slide], renderWidth: layer.renderWidth, focus: rig?.focus??projectorFocus,
    ...feedbackDeck.config(projectorDeck?.enabled || timed), keepPresenting:!!projectorDeck?.enabled };
}
const feedbackDeck = new VideoFeedbackControls(byId('video-feedback'), () => nativeSurface?.sync());
function anchorFeedback(discontinuity = false, ended = false) {
  feedbackDeck.clock({loaded:!!(state.audioBuffer||state.audioRef), playing:state.playing,
    position:state.audioBuffer?currentPosition():state.position,ended},discontinuity);
  if(discontinuity) projectorDeck?.flush();
  if (projectorDeck?.enabled || state.layers.some(l => l.envelope != null || l.media)) nativeSurface?.sync();
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
  if (Number.isSafeInteger(stats.simulationTick)) readout.textContent += ' · fluid step ' + stats.simulationTick;
}

const nativeSurface = nativeDesktop ? new NativeWheelSurface(
  window.__TAURI__.core.invoke, preview, liveSelection, reportNativeStats
) : null;
async function prepareWheel(){
  if(!liveSelection()) throw new Error('Select a visible native slide or artwork track first.');
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
  selection: wheelGestureSelection,
  prepare:prepareWheel,
  seek, onEdit: checkpoint,
}) : null;

const localLyrics = window.__TAURI__?.core ? installLocalLyrics({
  invoke: window.__TAURI__.core.invoke,
  apply: macroDeck ? groups => macroDeck.applyLyricGroups(groups) : null,
}) : null;

if (window.__TAURI__?.core) installVisualAdvisor({
  invoke:window.__TAURI__.core.invoke, source:()=>state.audioBuffer,
  selection:()=>selectedTimeRange(), apply:value=>applyTimelineSuggestion(value),
  selectedLyrics:range=>localLyrics?.excerpt(range) ?? '',
});

const wheelExporter = nativeDesktop ? new NativeWheelExport({
  invoke: window.__TAURI__.core.invoke, listen: window.__TAURI__.event.listen, selection: liveSelection,
  source: () => ({audioPath: state.audioPath, duration: state.audioBuffer?.duration, proceduralBusy: state.exporting || !!plateExporter?.busy || solvingProgram() || !!legends.audition}),
  getEvents: (id, duration) => macroDeck?.exportEvents(id, duration) ?? songGestureEvents(
    detachedGestures.macros, [...detachedGestures.audioGroups, ...detachedGestures.lyricGroups], id, duration),
  finishRecording: async () => { await macroDeck?.finishPendingRecording(); },
  pause, prepare: async () => { nativeSurface.exporting = true; await prepareWheel(); },
  sync: () => { nativeSurface.exporting = false; return nativeSurface.sync(); }, status,
}) : null;

const plateFormat = windowsDesktop ? new PlateFormatControls(byId('plate-formats'), async () => {
  await prepareWheel();
  return window.__TAURI__.core.invoke('preview_wheel_plates',{selection:liveSelection()});
},status,layout => window.__TAURI__.core.invoke("print_geometry",{layout})) : null;
const plateExporter = windowsDesktop ? new NativeWheelPlates({
  button: byId('export-wheel-plates'), format: () => plateFormat.options(), invoke: window.__TAURI__.core.invoke, listen: window.__TAURI__.event.listen, selection: wheelGestureSelection,
  busy: () => state.exporting || !!wheelExporter?.busy || solvingProgram() || usesLiveProjection(), prepare: prepareWheel, pause, status,
  refresh: updateTransport,
}) : null;

const performerDeck = windowsDesktop ? installPerformerDeck({
  invoke:window.__TAURI__.core.invoke, listen:window.__TAURI__.event.listen,
  selection:wheelGestureSelection, prepare:prepareWheel,
  layers:()=>state.layers.filter(isWheel),
}) : null;

function status(message) {
  byId('app-status').textContent = message;
  if (String(message).includes('Open FFmpeg media tools') && !byId('media-tools').hidden) byId('media-tools').open = true;
}
function formatTime(seconds) {
  const total = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`;
}
function currentPosition() {
  if (!state.audioBuffer) return 0;
  if (!state.playing) return state.position;
  return Math.min(state.audioBuffer.duration, state.position + state.audioContext.currentTime - state.startedAt);
}
function updateExportNote() {
  const note=byId('live-export-note');
  note.hidden=!nativeDesktop || !(solvingProgram() || usesLiveProjection());
  note.textContent=solvingProgram()?'Live fluid movie and plate export are unavailable.'
    : legends.audition?'Apply or stop the audition to export.'
    : 'Use native movie export to include projector lighting. Individual plate export is unavailable with this lighting.';
}
function updateTransport() {
  localLyrics?.source(state.audioPath, state.audioBuffer);
  const ready = !!state.audioBuffer;
  byId('play').disabled = !ready;
  byId('restart').disabled = !ready;
  byId('stop').disabled = !ready;
  updateExportNote();
  byId('export').disabled = !state.exporting && (!ready || !state.audioPath || !window.__TAURI__?.core || !!plateExporter?.busy || solvingProgram() || usesLiveProjection());
  wheelExporter?.refresh();
  plateExporter?.refresh();
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
async function prepareAudio(file, token, expected, selectionId = null) {
  const audio = ensureAudio();
  const bytes = await file.arrayBuffer();
  let sha256 = await songFingerprint(bytes).catch(() => null);
  if (!sha256 && selectionId != null) sha256 = await desktopInvoke('project_audio_digest', {selectionId});
  if (expected && sha256 !== expected.sha256) throw Error('This file does not match the audio saved with the project.');
  const decoded = await audio.decodeAudioData(bytes);
  if (token !== state.fileToken) return null;
  const analysis = await analyzeAudio(decoded, progress => status('Analyzing audio ' + Math.round(progress * 100) + '%'), () => token === state.fileToken);
  if (!analysis || token !== state.fileToken) return null;
  return {decoded, analysis, ref:{name:file.name, sha256}};
}
function installAudio(data, ref, path = null, selectionId = null, position = 0) {
  pause();
  state.audioBuffer = data?.decoded ?? null;
  state.analysis = data?.analysis ?? null;
  if (nativeDesktop) {
    const analysis=state.analysis;
    desktopInvoke('set_visual_audio_features',{fps:analysis?.fps??30,frames:analysis?.frames.map(f=>[f.bass,f.mid,f.treble,f.energy])??[]}).catch(e=>status(String(e)));
  }
  state.audioRef = ref;
  state.audioName = ref?.name ?? '';
  state.audioPath = path;
  state.audioSelectionId = selectionId;
  state.position = data ? Math.min(position, data.decoded.duration) : position;
  anchorFeedback(true);
  byId('track-title').textContent = projectName || ref?.name || 'Untitled session';
  byId('source-name').textContent = ref?.name || 'No audio selected';
  byId('source-meta').textContent = data
    ? formatTime(data.decoded.duration) + ' · ' + Math.round(data.decoded.sampleRate / 1000) + ' kHz · ' + (data.decoded.numberOfChannels === 1 ? 'Mono' : 'Stereo')
    : ref ? 'Audio offline · select the matching recording' : 'Open an MP3 or M4A to begin';
  byId('empty-wave').hidden = !!data;
  byId('relink-audio').hidden = !ref || !!data;
  drawWaveform();
  renderTimeline(); updateTransport();
}
async function loadAudio(file, beforeCommit = async () => {}, path = null, selectionId = null, relinking = false) {
  if (!file) return false;
  const token = ++state.fileToken;
  status('Reading audio…');
  try {
    const data = await prepareAudio(file, token, relinking ? state.audioRef : null, selectionId);
    if (!data || token !== state.fileToken) return false;
    await macroDeck?.stopForEdit();
    if (token !== state.fileToken) return false;
    await beforeCommit();
    if (token !== state.fileToken) return false;
    if(!relinking) { state.range=null; state.savedDuration=data.decoded.duration; }
    installAudio(data, data.ref, path, selectionId, relinking ? state.position : 0);
    if (!relinking) {
      detachedGestures = emptyGestures();
      if (macroDeck) {
        try { await macroDeck.songLoaded(data.ref.sha256, data.analysis, () => token === state.fileToken); }
        catch(error) { macroDeck.reportError(error); }
      } else detachedGestures.audioGroups = audioCueGroups(data.analysis);
      if (token !== state.fileToken) return false;
      resetHistory('Open audio');
    }
    status(relinking ? 'Project audio relinked' : 'Audio loaded');
    return true;
  } catch (error) {
    if (token === state.fileToken) status('Could not open audio: ' + (error.message || error));
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
function selectedLayer() { return state.layers.find((layer) => layer.id === state.selected); }
function applyPalette(layer, name, colors, dyes) {
  const members = layer.presetGroup == null ? [layer] :
    state.layers.filter(l => l.presetGroup === layer.presetGroup);
  if (members.some(l => l.filmDepth != null && (l.dyeIndex ?? 0) >= dyes.length)) {
    status('Choose this slide under New track to add its dye layers.');
    renderControls();
    return;
  }
  for (const member of members) {
    member.palette = name;
    member.colors = member.filmDepth == null ? [...colors] : [dyes[member.dyeIndex ?? 0]];
    member.customColors = false;
  }
  renderTimeline();
  renderControls();
}

async function loadSlidePresets() {
  try {
    const response = await fetch(new URL('./assets/slide-presets.json', import.meta.url));
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const { version, wheels } = await response.json();
    if (version !== 1 || !Array.isArray(wheels) || wheels.length === 0) throw new Error('Invalid slide list');
    const presets = wheels.map(({ id, name, swatches, dyes }) => {
      const colors = [swatches.carrier, swatches.rear, swatches.overlap, swatches.front];
      if (!/^[a-z]+(?:-[a-z]+)+$/.test(id) || !name || !Array.isArray(dyes) || !dyes.length ||
          [...colors,...dyes].some(color => !/^#[0-9a-f]{6}$/i.test(color))) {
        throw new Error('Invalid slide preset');
      }
      return { id, name, colors, dyes };
    });
    for (const preset of presets) {
      slidePresets.set(preset.id, preset);
      slideSelect.add(new Option(preset.name, preset.id));
    }
    if (nativeDesktop) {
      const benchmark = await window.__TAURI__.core.invoke('live_wheel_benchmark_config');
      // Historical benchmarks replay the original recorded fields by design.
      recordedBenchmark = !!benchmark;
      solverAvailable = !benchmark && (await window.__TAURI__.core.invoke('live_simulation_available').catch(() => false)) === true;
      byId('wheel-motion').querySelector('[value="simulated"]').disabled = !solverAvailable;
      if (!state.layers.length) {
        if (benchmark) addLayer();
        else addDyeSlide(slidePresets.get('violet-blue'));
      }
      if (benchmark) {
        await window.__TAURI__.event.listen('tss-benchmark-release', () => byId('replay-release').click());
        await window.__TAURI__.event.listen('tss-benchmark-wheel', ({ payload }) => {
          for (const [id, value] of [['wheel-slide', payload.slide], ['wheel-gesture', payload.gesture]]) {
            const control = byId(id);
            control.value = value;
            control.dispatchEvent(new Event('change'));
          }
          chooseSlidePreset(payload.wheel);
        });
      }
      const layer = selectedLayer();
      if (layer && benchmark) {
        layer.slide = benchmark?.slide ?? 'rear';
        layer.gesture = benchmark?.gesture ?? 'tug';
        layer.renderWidth = benchmark?.renderWidth ?? 640;
        chooseSlidePreset(benchmark.wheel ?? 'violet-blue');
        {
          const control = byId('wheel-motion');
          control.value = benchmark.motion;
          control.dispatchEvent(new Event('change'));
          if (benchmark.focus != null) {
            const focus = byId('wheel-focus');
            focus.value = String(Math.round(100 * benchmark.focus));
            focus.dispatchEvent(new Event('input'));
          }
        }
        if (benchmark?.wheelStack) {
          byId('add-layer').click();
          chooseSlidePreset('yellow-red');
          selectLayer(layer.id);
        }
        if (benchmark?.verifyPlates) await plateExporter.start();
        if (benchmark?.verifyExport || benchmark?.mediaTrial) {
          const grants = await desktopInvoke('proof_visual_grants');
          if (grants.length) {
            const [source, matte] = grants.map(newMedia);
            source.mode='gradient'; source.keyEnabled=true;
            let matteId=null;
            if (matte) { const mask=addArtwork(matte); mask.visible=false; matteId=mask.id; }
            source.maskA=matteId;
            addArtwork(source);
            selectLayer(layer.id);
          }
        }
        if (benchmark?.verifyExport) {
          const { verifyWheelExport } = await import('./verify-wheel-export.js');
          if (!windowsDesktop) nativeSurface.exporting = true;
          try {
            await nativeSurface.sync();
            await verifyWheelExport(liveSelection(), () => state.audioBuffer?.duration, status, projectorDeck,performerDeck);
          } finally {
            if (!windowsDesktop) nativeSurface.exporting = false;
          }
        }
      }
    }
  } catch (error) {
    slideSelect.options[0].textContent = 'Slide presets unavailable';
    status(`Could not read oil slides: ${error.message}`);
  } finally {
    recordedBenchmark = false;
    startingNative = false;
    renderTimeline(); renderControls();
  }
}

function chooseSlidePreset(id) {
  if (!slidePresets.has(id)) return;
  slideSelect.value = id;
  slideSelect.dispatchEvent(new Event('change'));
}

function makeLayer(source) {
  const id = Math.max(state.nextId, highestId + 1);
  highestId = id;
  state.nextId = id + 1;
  const layer = source ? structuredClone(layerSettings(source)) : {
    name: `Liquid ${id}`, visible: true, palette: 'orchid', colors: [...LIQUID_PALETTES.orchid], customColors: false, hue: 0, speed: 1, scale: 1, opacity: state.layers.length ? 0.7 : 1,
    visibleFilms: 'both', slide: 'rear', gesture: 'tug', tugResponse: { rear: 0.4, front: 0.4 }, mousePressure: { rear: 0.5, front: 0.5 }, renderWidth: 640
  };
  if (!source && selectedLayer()?.projector==='b') layer.projector='b';
  if (!recordedBenchmark) {
    layer.simulationSeed ??= newFluidSeed();
    if (layer.filmDepth == null) layer.colors = [source ? layer.colors[1] : LIQUID_PALETTES.orchid[0]];
    layer.filmDepth ??= .5;
    layer.visibleFilms = 'rear';
    layer.slide = 'rear';
    delete layer.presetGroup;
    delete layer.dyeIndex;
    delete layer.materialMotion;
  }
  return {...layer, id, nativeLive: nativeDesktop, motion:'wheel-flow', motionTake:0};
}
function addDyeSlide(preset, envelope = null) {
  const presetGroup = newPrefix();
  const layers = preset.dyes.map((color, dyeIndex) => {
    const layer = makeLayer();
    const depth = (dyeIndex + 1) / (preset.dyes.length + 1);
    return {...layer, name:`Oil ${layer.id}`, palette:preset.id, colors:[color], filmDepth:depth,
      dyeIndex, presetGroup, opacity:1, envelope:structuredClone(envelope)};
  });
  state.layers.unshift(...[...layers].reverse());
  selectLayer(layers[0].id);
  status(`Added ${preset.name} with ${layers.length} oil layers.`);
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
  button.addEventListener('click', (event) => { event.stopPropagation(); action(); checkpoint(label); });
  return button;
}
function renderTrack(layer,index,tracks,list,lanes) {
  const row = document.createElement('div');
  row.className = `layer-row${layer.id === state.selected ? ' selected' : ''}`;
  row.tabIndex = 0; row.setAttribute('role', 'button'); row.setAttribute('aria-label', `Select ${layer.name}`);
  row.addEventListener('click', () => selectLayer(layer.id));
  row.addEventListener('keydown', (event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectLayer(layer.id); } });
  const icon = document.createElement('span'); icon.className = 'layer-icon'; icon.textContent = '◉';
  if (layer.filmDepth != null) icon.style.color = layer.colors[0];
  const info = document.createElement('div'); info.className = 'layer-text';
  const title = document.createElement('strong'); title.textContent = layer.name;
  const subtitle = document.createElement('small'); subtitle.textContent = layer.visible ? (layer.media ? 'Artwork' : 'Liquid light') : 'Hidden';
  info.append(title, subtitle);
  const toggle = layerButton(layer.visible ? '●' : '○', `${layer.visible ? 'Hide' : 'Show'} ${layer.name}`, () => { layer.visible = !layer.visible; renderTimeline(); renderControls(); });
  function swap(other) {
    const a=state.layers.indexOf(layer),b=state.layers.indexOf(other);
    [state.layers[a],state.layers[b]]=[other,layer];renderTimeline();
  }
  const up = layerButton('↑', `Move ${layer.name} up`, () => swap(tracks[index-1]), index===0);
  const down = layerButton('↓', `Move ${layer.name} down`, () => swap(tracks[index+1]), index===tracks.length-1);
  const remove = layerButton('×', `Remove ${layer.name}`, () => {
    state.layers.splice(state.layers.indexOf(layer), 1);
    state.renderError = null;
    byId('render-error').hidden = true;
    if (state.selected === layer.id) state.selected = state.layers[0]?.id ?? null;
    renderTimeline(); renderControls(); status(`${layer.name} removed`);
    if (nativeSurface) retainedNativeLayers.add(layer.id);
  });
  row.append(icon, info, toggle, up, down, remove); list.append(row);
  const lane = document.createElement('div'); lane.className = 'visual-lane';
  const clip = document.createElement('div'); clip.className = `visual-clip${layer.visible ? '' : ' hidden'}`;
  const clipName = document.createElement('span'); clipName.textContent = layer.name;
  const clipDuration = document.createElement('small'); clipDuration.textContent = state.audioBuffer ? formatTime(state.audioBuffer.duration) : 'Full track';
  if(layer.envelope==null) {clip.append(clipName, clipDuration); lane.append(clip);}
  else for(const interval of layer.envelope) {
    const block=document.createElement('button');block.type='button';
    block.className=clip.className+' timed-clip';
    const duration=timelineDuration()||interval.end;
    block.style.left=(100*interval.start/duration)+'%';block.style.width=(100*(interval.end-interval.start)/duration)+'%';
    block.textContent=interval.start.toFixed(2)+'–'+interval.end.toFixed(2);
    block.setAttribute('aria-label',layer.name+' '+block.textContent+' seconds');
    block.addEventListener('click',()=>{
      selectLayer(layer.id); setTimeRange(interval.start,interval.end);
      if(interval.rule)restoreWandRule(interval.rule);
      byId('wand-length').value=interval.end-interval.start;
      byId('wand-in').value=interval.fadeIn;byId('wand-out').value=interval.fadeOut;byId('wand-level').value=interval.level;
      byId('timeline-wand').open=true;
    });lane.append(block);
  }
  lanes.append(lane);
}
function renderTimeline() {
  const list = byId('layer-list'), lanes = byId('visual-lanes');
  list.replaceChildren(); lanes.replaceChildren();
  const beams = lighting.grouped ? ['a','b'] : ['a'];
  for(const projector of beams) {
    if(lighting.grouped) {
      const heading=document.createElement('div');heading.className='projector-heading';heading.dataset.projector=projector;
      heading.textContent=lighting.groupLabel(projector);list.append(heading);
      const divider=document.createElement('div');divider.className='projector-heading';divider.setAttribute('aria-hidden','true');lanes.append(divider);
    }
    const tracks=state.layers.filter(l=>(l.projector??'a')===projector);
    tracks.forEach((layer, index) => {
      renderTrack(layer,index,tracks,list,lanes);
    });
  }
  nativeSurface?.sync();
  refreshTimelineTools();
  byId('layer-count').textContent = `${state.layers.length} visual ${state.layers.length === 1 ? 'layer' : 'layers'}`;
  const ruler = byId('ruler'); ruler.replaceChildren();
  for (let i = 0; i <= 8; i++) {
    const tick = document.createElement('span'); tick.textContent = state.audioBuffer ? formatTime(state.audioBuffer.duration * i / 8) : '--:--'; ruler.append(tick);
  }
}
function renderControls() {
  byId('projection-light').hidden=!nativeDesktop;
  lighting.refresh();
  updateExportNote();
  visualMedia?.refresh();
  const layer = selectedLayer();
  byId('selected-name').textContent = layer?.name || 'No layer selected';
  const grid = byId('palette-grid'); grid.replaceChildren();
  Object.entries(LIQUID_PALETTES).forEach(([name, colors]) => {
    const button = document.createElement('button'); button.className = `palette${layer?.palette === name && !layer?.customColors ? ' active' : ''}`;
    button.title = paletteNames[name] || name; button.setAttribute('aria-label', paletteNames[name] || name);
    button.setAttribute('aria-pressed', String(layer?.palette === name && !layer?.customColors)); button.disabled = !layer || !!layer.media;
    colors.forEach((color) => { const swatch = document.createElement('span'); swatch.style.backgroundColor = color; button.append(swatch); });
    button.addEventListener('click', () => { layer.nativeLive = nativeDesktop; applyPalette(layer, name, colors, colors); }); grid.append(button);
  });
  const wheel = layer && !layer.media && slidePresets.get(layer.palette);
  const paletteAvailable = !!wheel || (!!layer && !layer.media && Object.hasOwn(LIQUID_PALETTES, layer.palette));
  const live = liveSelection();
  byId('plate-formats').hidden = !plateExporter || !live || !!layer.media || solvingProgram() || usesLiveProjection();
  wheelExporter?.refresh();
  plateExporter?.refresh();
  byId('video-feedback').hidden = !live;
  byId('native-wheel-controls').hidden = !nativeDesktop || !paletteAvailable;
  byId('native-performance').hidden = !live;
  byId('native-wheel-controls').hidden ||= !!layer?.media;
  byId('live-wheel').checked = !!live;
  byId('live-wheel').disabled = !paletteAvailable || !layer?.visible || liveFluidTrack(layer);
  const simulated = liveFluidTrack(layer);
  const single = layer?.filmDepth != null;
  byId('film-depth').parentElement.hidden = !single;
  if (single) {
    byId('film-depth').value = Math.round(layer.filmDepth * 100);
    byId('film-depth-value').textContent = byId('film-depth').value+'%';
  }
  const motionControl = byId('wheel-motion');
  motionControl.value = simulated ? 'simulated' : layer?.motion ?? 'wheel-flow';
  motionControl.disabled = simulated || !live;
  motionControl.parentElement.hidden = simulated;
  byId('rear-film-oil').closest('.native-choices').hidden = simulated;
  const coral = layer?.motion === 'coral-release', beads = layer?.motion === 'wheel-with-coral-beads';
  for (const slide of ['rear','front']) {
    const control = byId(slide+'-film-oil');
    control.value = layer?.materialMotion?.[slide] ?? 'wheel';
    control.disabled = !live || liveFluidTrack(layer) || coral || (beads && slide==='front');
  }
  const oilNote = byId('film-oil-note');
  oilNote.hidden = single || !(coral || beads || liveFluidTrack(layer));
  oilNote.textContent = liveFluidTrack(layer) ? 'The front and rear cells simulate independently. Drag to move the selected material.'
    : coral ? 'Coral release supplies both films. Slide flow restores their oil selections.'
    : beads ? 'The front film uses the coral bead release. Slide flow restores its oil selection.' : '';
  byId('wheel-films').value = layer?.visibleFilms ?? 'both';
  byId('wheel-films').disabled = !live || single;
  byId('wheel-films').closest('.native-choices').hidden = single;
  byId('replay-release').hidden = !hasRelease(layer?.motion);
  byId('replay-release').textContent = layer?.motion === 'wheel-with-coral-beads'
    ? 'Replay front beads' : 'Replay release live';
  byId('replay-release').disabled = !live;
  byId('wheel-slide').value = layer?.slide ?? 'rear';
  byId('wheel-width').value = String(layer?.renderWidth ?? 640);
  byId('wheel-gesture').value = layer?.gesture ?? 'tug';
  byId('wheel-gesture').disabled = byId('wheel-slide').disabled = byId('wheel-width').disabled = !live;
  byId('wheel-slide').disabled ||= single;
  byId('wheel-slide').parentElement.hidden = single;
  byId('tug-response').value = String(layer?.tugResponse[layer.slide] ?? 0.4);
  byId('tug-response').disabled = !live || layer.gesture !== 'tug';
  byId('mouse-pressure-controls').hidden = !live || layer.gesture !== 'pressure';
  const strength = Math.round(100 * (layer?.mousePressure?.[layer?.slide] ?? .5));
  byId('mouse-pressure').value = strength;
  byId('mouse-pressure-value').textContent = strength+'%';
  byId('wheel-focus').disabled = !live;
  updateFocusControl();
  preview.parentElement.classList.toggle('native-active', !!live);
  byId('preview-status').textContent = live?.stack?.length > 1 ? `NATIVE GPU · ${live.stack.filter(l=>!l.matteOnly).length} TRACKS` : live?.stack?.some(l=>l.media) ? 'NATIVE GPU · ARTWORK' : live ? 'RECORDED SLIDE STUDY' : 'PROCEDURAL PREVIEW';
  if (liveFluidTrack(layer) && live) byId('preview-status').textContent = 'LIVE FLUID SIMULATION';
  byId('solver-unavailable').hidden = !missingSolver();
  if (missingSolver()) byId('preview-status').textContent = 'LIVE FLUID SIMULATION UNAVAILABLE';
  const target = single ? 'selected layer' : (layer?.slide ?? 'rear')+' slide';
  byId('native-instruction').textContent = layer?.media ? layer.media.name : layer?.gesture === 'rotate'
    ? `Drag around the center to rotate the ${target}.`
    : layer?.gesture === 'pressure' ? `Press the ${target} to compress it. Release to restore the gap.`
    : `Drag to tug the oil in the ${target}.`;
  byId('audio-note').hidden = !!live;
  byId('hue').disabled = !layer || !!layer.media;
  byId('speed').disabled = !layer || !!layer.media || (!!live && !liveFluidTrack(layer));
  byId('speed').title = liveFluidTrack(layer) ? 'Simulated seconds per second. Zero pauses the fluid.' : live ? 'This recorded study has a fixed timebase.' : 'Procedural animation rate';
  byId('scale').disabled = !layer || !!layer.media;
  byId('opacity').disabled = !layer;
  byId('export').title = 'Export the procedural timeline composite';
  if (!state.exporting) byId('export').textContent = live ? 'Export procedural preview' : 'Export 4K preview';
  if (live && !nativeSurface.surfaceRequested) byId('native-frame-rate').textContent = 'Starting native slide…';
  nativeSurface?.sync();
  slideSelect.disabled = !layer || !!layer.media || slidePresets.size === 0;
  slideSelect.value = wheel && !layer.customColors ? wheel.id : '';
  if (slidePresets.size) slideSelect.options[0].textContent = wheel && layer.customColors ? `Custom · ${wheel.name}` : 'Choose a slide';
  byId('pigment-heading').textContent = single ? 'LAYER DYE' : wheel ? 'SLIDE SWATCHES' : 'FILM PIGMENTS';
  const pigments = byId('pigment-colors'); pigments.replaceChildren();
  if (layer) layer.colors.forEach((color, index) => {
    const label = document.createElement('label');
    const name = single ? 'Dye' : wheel ? ['Clear', 'Rear slide', 'Overlap', 'Front slide'][index] : ['Rear oil', 'Front oil', 'Rear rim', 'Front rim'][index];
    label.textContent = name;
    const picker = document.createElement('input');
    picker.type = 'color'; picker.value = color;
    picker.disabled = !!layer.media;
    picker.setAttribute('aria-label', `${name} color`);
    picker.addEventListener('input', () => { layer.colors[index] = picker.value; layer.customColors = true; nativeSurface?.sync(); });
    picker.addEventListener('change', () => { renderTimeline(); renderControls(); checkpoint('Set film pigment'); });
    label.append(picker); pigments.append(label);
  });
  if (!layer) return;
  ['hue', 'speed', 'scale', 'opacity'].forEach((id) => { byId(id).value = layer[id]; });
  byId('hue-label').textContent = `${layer.hue}°`;
  byId('speed-output').textContent = `${layer.speed.toFixed(1)}×`;
  byId('scale-output').textContent = `${layer.scale.toFixed(2)}×`;
  byId('opacity-output').textContent = `${Math.round(layer.opacity * 100)}%`;
}
function renderFrame(timestamp) {
  macroDeck?.tick();
  if (!startingNative && !solvingProgram() && !liveSelection() && !nativeSurface?.surfaceRequested &&
      !state.renderError && !state.exporting && !document.hidden && !document.querySelector('dialog[open]')) {
    try {
      proceduralPreview ??= new LiquidRenderer(preview, {composite:true});
      proceduralPreview.gl.clear(proceduralPreview.gl.COLOR_BUFFER_BIT);
      const time = state.audioBuffer ? currentPosition() : state.audioRef ? state.position : timestamp / 1000;
      const features = featuresAt(state.analysis, time);
      const exposed = [];
      for (const layer of state.layers) {
        if (!layer.visible || layer.media) continue;
        const opacity = layer.opacity * opacityAt(layer.envelope,time);
        if (opacity > 0) exposed.push({layer, opacity});
        if (opacity === 1) break;
      }
      for (const {layer, opacity} of exposed.reverse()) {
        const palette = layer.colors.map((color) => rotateColor(color, layer.hue));
        proceduralPreview.render({time, ...features, palette, speed:layer.speed, scale:layer.scale, opacity});
      }
    } catch (error) { showRenderError(error); }
  }
  if (state.playing) updateTransport();
  requestAnimationFrame(renderFrame);
}
const desktopInvoke = window.__TAURI__?.core?.invoke;
installMediaTools(byId('media-tools'), desktopInvoke);
function byteFile(name, bytes) {
  return new File([bytes instanceof ArrayBuffer || ArrayBuffer.isView(bytes) ? bytes : new Uint8Array(bytes)], name);
}
window.tssLoadAudioBytes = (name, bytes, beforeCommit) => loadAudio(byteFile(name, bytes), beforeCommit);
let audioBusy = false;
if (desktopInvoke) {
  window.tssImportAudio = async (grantedSelection, relinking = false) => {
    if (audioBusy || projectBusy) return;
    audioBusy = true;
    byId('open-audio').disabled = byId('relink-audio').disabled = true;
    try {
      const selection = Array.isArray(grantedSelection) ? grantedSelection : await desktopInvoke('choose_audio');
      if (!selection) return;
      const [selectionId, path] = selection;
      const bytes = await desktopInvoke('read_pending_audio', {selectionId});
      const name = path.split(/[\\/]/).pop();
      if (await loadAudio(byteFile(name, bytes), () => desktopInvoke('confirm_audio', {selectionId}), path, selectionId, relinking)) {
        const token = state.fileToken;
        if (state.audioRef.sha256) {
          try { await desktopInvoke('remember_project_audio', {selectionId, sha256:state.audioRef.sha256}); }
          catch(error) { status('Audio loaded. Could not remember its location: ' + error); }
        }
        if (macroDeck && !relinking && token === state.fileToken) {
          try {
            const sidecar = await desktopInvoke('read_timed_lyrics',{selectionId});
            if (token === state.fileToken) macroDeck.importSidecar(sidecar);
          } catch(error) { if(token === state.fileToken) macroDeck.reportError(error); }
        }
      }
    } finally {
      byId('open-audio').disabled = byId('relink-audio').disabled = false;
      audioBusy = false;
    }
  };
}

async function exportVideo() {
  if (usesLiveProjection() && !state.exporting) throw Error('Projector lighting and routing require live output.');
  if (solvingProgram() && !state.exporting) throw Error('Live solver trajectories are not yet supported by movie export.');
  if (state.exporting) {
    state.cancelExport = true;
    status('Cancelling export…');
    if (state.exportFinalizing) await window.__TAURI__.core.invoke('cancel_export');
    return;
  }
  if (!state.audioPath || !state.audioBuffer || !window.__TAURI__?.core || plateExporter?.busy) return;
  const invoke = window.__TAURI__.core.invoke;
  const outputPath = await invoke('choose_export_path');
  if (!outputPath) return;
  const audioPath = state.audioPath;
  const audioDuration = state.audioBuffer.duration;
  const analysis = state.analysis;
  const layers = state.layers.filter((layer) => layer.visible && !layer.media).map((layer) => ({
    colors: [...layer.colors], hue: layer.hue, speed: layer.speed, scale: layer.scale, opacity: layer.opacity, envelope:layer.envelope,
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
        output.globalAlpha = layer.opacity * opacityAt(layer.envelope,time);
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
    byId('audio-file').dataset.relink = ''; byId('audio-file').click();
  }
});
byId('relink-audio').addEventListener('click', () => {
  if (desktopInvoke) window.tssImportAudio(undefined, true).catch(error => status('Could not relink audio: ' + error));
  else { byId('audio-file').dataset.relink = 'true'; byId('audio-file').click(); }
});
byId('audio-file').addEventListener('change', async (event) => {
  const file = event.target.files?.[0], relink = event.target.dataset.relink === 'true';
  event.target.value = '';
  if (audioBusy || projectBusy) return;
  audioBusy = true;
  try { await loadAudio(file, undefined, null, null, relink); }
  finally { audioBusy = false; }
});
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
  if (layer && layer.filmDepth == null) { layer.slide = event.target.value; renderControls(); }
});
for (const slide of ['rear','front']) {
  byId(slide+'-film-oil').addEventListener('change', event => {
    const layer = selectedLayer();
    if (!layer || layer.media || event.target.disabled) return;
    layer.materialMotion = {...layer.materialMotion, [slide]:event.target.value};
    nativeSurface?.sync();
    checkpoint('Set '+slide+' film oil');
  });
}
byId('wheel-films').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (layer && layer.filmDepth == null) { layer.visibleFilms = event.target.value; nativeSurface?.sync(); }
});
byId('wheel-motion').addEventListener('change', (event) => {
  const layer = selectedLayer();
  if (!layer || liveFluidTrack(layer) || layer.media) return;
  if (event.target.value === 'simulated') {
    if (!solverAvailable) return;
    const front = layer.slide === 'front';
    layer.colors = [layer.colors[front ? 3 : 1]];
    layer.filmDepth = front ? 2/3 : 1/3;
    layer.slide = layer.visibleFilms = 'rear';
    delete layer.materialMotion;
    layer.simulationSeed = newFluidSeed();
    layer.nativeLive = true;
    layer.motion = 'wheel-flow';
    renderControls();
    return;
  }
  if (layer.motion === event.target.value) return;
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
  const hasIndependentOil = state.layers.some(l=>l.filmDepth != null);
  const label = hasIndependentOil ? value+'%' : projectorFocus === null ? 'Wheel setting'
    : value === 0 ? 'Rear slide' : value === 100 ? 'Front slide' : `${value}% toward front`;
  byId('wheel-focus').value = value;
  byId('wheel-focus').setAttribute('aria-valuetext', label);
  byId('wheel-focus-value').textContent = label;
  byId('wheel-focus-reset').disabled = projectorFocus === null || !liveSelection();
}
byId('film-depth').addEventListener('input', event => {
  const layer = selectedLayer();
  if (layer?.filmDepth == null) return;
  layer.filmDepth = Number(event.target.value) / 100;
  byId('film-depth-value').textContent = event.target.value+'%';
  nativeSurface?.sync();
});
byId('wheel-focus').addEventListener('input', (event) => {
  projectorFocus = Number(event.target.value) / 100;
  updateFocusControl(); nativeSurface?.sync();
});
byId('wheel-focus-reset').addEventListener('click', () => {
  projectorFocus = null;
  updateFocusControl(); nativeSurface?.sync();
});
slideSelect.addEventListener('change', () => {
  const layer = selectedLayer();
  const preset = slidePresets.get(slideSelect.value);
  if (layer && preset) { layer.nativeLive = nativeDesktop; applyPalette(layer, preset.id, preset.colors, preset.dyes); }
});
byId('play').addEventListener('click', () => { if (state.playing) pause(); else play().catch((error) => status(`Playback failed: ${error.message}`)); });
byId('restart').addEventListener('click', () => seek(0).catch((error) => status(error.message)));
byId('stop').addEventListener('click', () => { pause(); state.position = 0; anchorFeedback(true); macroDeck?.transportChanged(); updateTransport(); status('Stopped'); });
byId('add-layer').addEventListener('click', () => {
  const source = byId('track-source').value;
  if(source==='blank')addLayer();
  else if (source.startsWith('slide:')) addDyeSlide(slidePresets.get(source.slice(6)));
  else projectOperation('Add saved or template track', addTrackFromSource);
});
const seekArea=byId('seek-area');
let drag=null, dragged=false, selectedGlyph=glyphs[0];
const selectionOverlay=document.createElement('div');
selectionOverlay.className='time-selection';selectionOverlay.hidden=true;seekArea.append(selectionOverlay);
function timelineDuration(){return state.audioBuffer?.duration||state.savedDuration||0;}
function selectedTimeRange(){return validRange(state.range)?{...state.range}:null;}
function setTimeRange(start,end,record=true) {
  const range={start,end};
  if(!validRange(range)||end>timelineDuration()+1e-8)throw Error('Select an interval within the song duration.');
  state.range={start,end:Math.min(end,timelineDuration())};paintRange();
  if(record) checkpoint('Select time range');
}
function selectedCues(){const d=macroDeck?.projectState()??detachedGestures;return [...d.audioGroups,...d.lyricGroups];}
function paintRange() {
  const duration=timelineDuration(), r=selectedTimeRange();
  byId('range-start').value=r?.start??0;byId('range-end').value=r?.end??duration;
  selectionOverlay.hidden=!r||!duration;
  if(r&&duration){selectionOverlay.style.left=(100*r.start/duration)+'%';selectionOverlay.style.width=(100*(r.end-r.start)/duration)+'%';}
}
function refreshTimelineTools() {
  paintRange();
  const cue=byId('wand-cue'), old=cue.value;
  cue.replaceChildren(new Option('Choose a recurring cue',''),...selectedCues().map(c=>new Option(c.label,c.id)));
  if([...cue.options].some(o=>o.value===old))cue.value=old;
  byId('boundary-glyphs').replaceChildren(...glyphs.map(g=>{
    const button=document.createElement('button');button.className='button quiet';button.type='button';
    button.textContent=g+' '+state.bindings[g];button.setAttribute('aria-pressed',String(g===selectedGlyph));
    button.addEventListener('click',()=>{selectedGlyph=g;refreshTimelineTools();});return button;
  }));
  byId('wand-mode').value=state.bindings[selectedGlyph];
  refreshTrackSources();
}
const field=id=>byId('wand-'+id);
const numeric=id=>field(id).valueAsNumber;
function wandRule() {
  return {glyph:selectedGlyph,mode:field('mode').value,length:numeric('length'),interval:numeric('interval'),fadeIn:numeric('in'),fadeOut:numeric('out'),
    startAnchor:field('start-anchor').value,endAnchor:field('end-anchor').value,anchorTolerance:numeric('anchor-tolerance'),
    level:numeric('level'),autoScale:field('autoscale').checked,tolerance:numeric('tolerance'),cue:field('cue').value,
    signal:field('signal').value,threshold:numeric('threshold'),gap:numeric('gap')};
}
function restoreWandRule(rule) {
  if(glyphs.includes(rule.glyph))selectedGlyph=rule.glyph;
  refreshTimelineTools();
  for(const [key,input] of Object.entries({mode:'mode',length:'length',interval:'interval',startAnchor:'start-anchor',
      endAnchor:'end-anchor',anchorTolerance:'anchor-tolerance',fadeIn:'in',fadeOut:'out',level:'level',cue:'cue',
      signal:'signal',threshold:'threshold',tolerance:'tolerance',gap:'gap'})) {
    if(rule[key]!==undefined)field(input).value=rule[key];
  }
  field('autoscale').checked=!!rule.autoScale;
}
function timelineAction(label,fn) {
  try {fn(); checkpoint(label);status(label);}catch(e){status(e.message||String(e));}
}
function selectedEditRange() {
  const r=selectedTimeRange();
  if(!r)throw Error('Select a waveform interval or choose Select all.');
  if(!selectedLayer())throw Error('Select a visual layer first.');
  return r;
}
function applyTimelineSuggestion(s) {
  const range=selectedTimeRange();
  if(!range || !validRange(s) || s.start<range.start || s.end>range.end)
    throw Error('Keep the reviewed timing within the selected range.');
  if(s.visual && s.visual!=='oil')throw Error('Choose an oil track for this action.');
  const rule=wandRule();
  if(s.fadeIn!==undefined)rule.fadeIn=s.fadeIn;
  if(s.fadeOut!==undefined)rule.fadeOut=s.fadeOut;
  const envelope=advisorIntervals({...s,signal:s.signal??'time',fit:s.fit??'elastic'},rule,selectedCues(),state.analysis);
  const source=selectedLayer()?.media?null:selectedLayer(), key=s.wheel??s.palette??source?.palette??'violet-blue';
  if(!slidePresets.has(key)&&!LIQUID_PALETTES[key])throw Error('Choose a known slide or palette.');
  if(s.opacity!==undefined&&(!Number.isFinite(s.opacity)||s.opacity<0||s.opacity>1))throw Error('Choose an opacity between 0 and 1.');
  const layer=makeLayer(source), id=layer.id;
  if(source?.palette!==key || s.wheel || s.palette){
    layer.palette=key;layer.colors=[(slidePresets.get(key)?.dyes??LIQUID_PALETTES[key])[0]];
    layer.customColors=false;layer.nativeLive=nativeDesktop;
  }
  Object.assign(layer,{id,name:s.label||s.cue||'Oil cue',visible:true,envelope,opacity:s.opacity??source?.opacity??1});
  state.layers.unshift(layer);selectLayer(id);checkpoint('Apply reviewed oil cue');
  return id;
}
function replaceSelected(rule) {
  const r=selectedEditRange(), layer=selectedLayer();
  const planned=rule?planIntervals(rule,r,{groups:selectedCues(),analysis:state.analysis}):[];
  if(rule&&!planned.length)throw Error('No matches in this selection. Choose a different cue, threshold or time boundary.');
  layer.envelope=replaceRegion(layer.envelope,r,planned,timelineDuration());
  renderTimeline();renderControls();
}
field('mode').addEventListener('change',()=>timelineAction('Set boundary behavior',()=>{
  state.bindings[selectedGlyph]=field('mode').value;refreshTimelineTools();
}));
byId('set-range').addEventListener('click',()=>timelineAction('Select time range',()=>{
  setTimeRange(byId('range-start').valueAsNumber,byId('range-end').valueAsNumber,false);
}));
byId('select-all-time').addEventListener('click',()=>timelineAction('Select entire song',()=>setTimeRange(0,timelineDuration(),false)));
byId('deselect-time').addEventListener('click',()=>timelineAction('Deselect time range',()=>{state.range=null;paintRange();}));
byId('apply-wand').addEventListener('click',()=>timelineAction('Apply boundary to selected layer',()=>replaceSelected(wandRule())));
byId('clear-region').addEventListener('click',()=>projectOperation('Clear selected layer in range',async()=>{
  const range=selectedEditRange(), layer=selectedLayer();
  await macroDeck?.stopForEdit();
  setGestureData(removeRegionGestures(gestureData(),layer.id,range,timelineDuration(),newPrefix()));
  replaceSelected(null);checkpoint('Clear selected layer in range');status('Cleared selected layer in range');
}));
function atWave(event) {
  const rect=seekArea.getBoundingClientRect();
  return Math.max(0,Math.min(1,(event.clientX-rect.left)/rect.width))*timelineDuration();
}
seekArea.addEventListener('pointerdown',event=>{
  if(event.button!==0||!timelineDuration())return;
  drag={id:event.pointerId,x:event.clientX,start:atWave(event),before:state.range};
  dragged=false;seekArea.setPointerCapture(event.pointerId);
});
seekArea.addEventListener('pointermove',event=>{
  if(drag?.id!==event.pointerId || (!dragged && Math.abs(event.clientX-drag.x)<4))return;
  const t=atWave(event);if(t===drag.start)return;
  dragged=true;setTimeRange(Math.min(t,drag.start),Math.max(t,drag.start),false);
});
seekArea.addEventListener('pointerup',event=>{if(drag?.id===event.pointerId){drag=null;if(dragged)checkpoint('Select time range');}});
seekArea.addEventListener('pointercancel',()=>{if(drag)state.range=drag.before;drag=null;dragged=false;refreshTimelineTools();});
seekArea.addEventListener('click',event=>{
  if(!dragged)seek(atWave(event)).catch(error=>status(error.message));
  dragged=false;
});
for (const id of ['hue', 'speed', 'scale', 'opacity']) {
  byId(id).addEventListener('input', (event) => {
    const layer = selectedLayer();
    if (!layer) return;
    layer[id] = Number(event.target.value);
    if (id === 'hue') { renderControls(); return; }
    byId(id+'-output').textContent = id === 'opacity' ? Math.round(layer.opacity*100)+'%' : layer[id].toFixed(2)+'×';
    nativeSurface?.sync();
  });
}
function authoredState() {
  for(const {media} of state.layers)if(media?.assetId!=null) {
    const {assetId:id,sha256,name,durationUs,width,height,still}=media;
    visualGrants.set(sha256,{id,sha256,name,durationUs,width,height,still});
  }
  return {timeline:{range:state.range,duration:timelineDuration(),bindings:state.bindings}, layers:state.layers.map(layerSettings), selected:state.selected, nextId:state.nextId,
    focus:projectorFocus, ...lighting.fields(), feedback:feedbackDeck.projectState(), exportBaseline:byId('native-export-baseline').value,
    gestures:macroDeck?.projectState() ?? detachedGestures};
}
function editSignature() { return JSON.stringify([state.audioRef, authoredState()]); }
function projectDocument() {
  return {format:PROJECT_FORMAT, version:1, audio:state.audioRef,
    position:state.audioBuffer ? currentPosition() : state.position, ...authoredState()};
}
function refreshHistory() {
  const menu = byId('edit-history');
  menu.replaceChildren(...editHistory.entries.map(({label}, i) => new Option(label, String(i))));
  menu.value = String(editHistory.cursor);
  byId('undo-edit').disabled = editHistory.cursor <= 0;
  byId('redo-edit').disabled = editHistory.cursor >= editHistory.entries.length - 1;
}
function checkpoint(label) {
  if (!editHistory.entries.length) return;
  editHistory.record(authoredState(), label);
  refreshHistory();
}
function resetHistory(label) { editHistory.reset(authoredState(), label); refreshHistory(); }
function restoreEdits(doc, newProject) {
  legends.stop();
  const old = new Map(state.layers.map(layer => [layer.id, layer]));
  state.layers = doc.layers.map(config => {
    const existing = newProject ? null : old.get(config.id);
    if (existing) old.delete(existing.id);
    const layer=Object.assign(existing || {}, {envelope:null,media:undefined,materialMotion:undefined,simulationSeed:undefined,
      filmDepth:undefined,presetGroup:undefined,dyeIndex:undefined,projector:undefined}, layerSettings(config));
    const grant=layer.media&&visualGrants.get(layer.media.sha256);
    if(grant)bindProjectMedia(layer,grant);
    return layer;
  });
  for (const layer of old.values()) {
    if (!newProject && nativeSurface) retainedNativeLayers.add(layer.id);
  }
  state.nextId = doc.nextId;
  state.range=doc.timeline?.range??null;
  state.savedDuration=doc.timeline?.duration??0;
  state.bindings=doc.timeline?.bindings??defaultBindings();
  state.selected = doc.selected;
  if (newProject) highestId = doc.nextId - 1;
  projectorFocus = doc.focus;
  lighting.restore(doc);
  feedbackDeck.restoreProject(doc.feedback);
  byId('native-export-baseline').value = doc.exportBaseline;
  if (macroDeck) macroDeck.restoreProject(doc.gestures, newProject);
  else detachedGestures = doc.gestures;
  state.renderError = null;
  byId('render-error').hidden = true;
  renderTimeline(); renderControls();
}
async function projectOperation(message, task) {
  if (projectBusy || audioBusy || state.exporting || wheelExporter?.busy) { status('Finish the current operation first.'); return; }
  projectBusy = true;
  const dialog = byId('project-operation');
  byId('project-message').textContent = message; dialog.showModal();
  nativeSurface?.sync();
  try { await presetsReady; await task(); }
  catch (error) { status(message + ' failed: ' + (error.message || error)); }
  finally { dialog.close(); projectBusy = false; nativeSurface?.sync(); }
}
function restoreHistory(cursor) {
  const doc = editHistory.stateAt(cursor);
  if (!doc || cursor === editHistory.cursor) return;
  return projectOperation('Restore edit', async () => {
    await macroDeck?.stopForEdit();
    restoreEdits(doc, false);
    editHistory.cursor = cursor; refreshHistory();
    status(editHistory.entries[cursor].label);
  });
}
function chooseReplacement() {
  if (editSignature() === savedEdits) return true;
  const dialog = byId('replace-project');
  return new Promise(resolve => {
    dialog.returnValue = '';
    dialog.addEventListener('close', () => resolve(dialog.returnValue === 'replace'), {once:true});
    dialog.showModal();
  });
}
const palettes = () => new Set([...Object.keys(LIQUID_PALETTES), ...slidePresets.keys()]);
const shippedTracks = new Map([
  ['wheel',{name:'Violet / Blue slide',palette:'violet-blue',motion:'wheel-flow',rule:{glyph:'[---]',mode:'stretch',fadeIn:.25,fadeOut:.4}}],
  ['lagoon',{name:'Lagoon liquid',palette:'lagoon',motion:'wheel-flow',rule:{glyph:'*---*',mode:'signal',signal:'bass',threshold:.18,fadeIn:.08,fadeOut:.2}}],
]);
const libraryCache=new Map();
let copySequence=0;
function newPrefix(){return (globalThis.crypto?.randomUUID?.()??Date.now()+'-'+(++copySequence))+'/';}
function libraryKey(kind){return 'tss.track-library.v1.'+(kind==='user'?'user':'song.'+(state.audioRef?.sha256??''));}
function library(kind) {
  if(kind==='song'&&!state.audioRef?.sha256)return [];
  const key=libraryKey(kind);
  if(!libraryCache.has(key)){
    const entries=JSON.parse(localStorage.getItem(key)||'[]');
    if(!Array.isArray(entries))throw Error('Invalid saved track library.');
    libraryCache.set(key,entries);
  }
  return libraryCache.get(key);
}
function refreshTrackSources() {
  const menu=byId('track-source'), selected=menu.value;
  const options=[new Option('Blank liquid layer','blank')];
  if (!recordedBenchmark) for (const preset of slidePresets.values()) {
    options.push(new Option(preset.name+' · '+preset.dyes.length+' oils','slide:'+preset.id));
  }
  for(const [key,s] of shippedTracks)options.push(new Option('Shipped · '+s.name,'shipped:'+key));
  try {
    for(const kind of ['song','user'])for(const entry of library(kind))options.push(new Option((kind==='song'?'Song · ':'Template · ')+entry.name,kind+':'+entry.id));
  }catch(error){status('Track library unavailable: '+error.message);}
  menu.replaceChildren(...options);
  if(options.some(o=>o.value===selected))menu.value=selected;
}
function gestureData(){return macroDeck?.projectState()??detachedGestures;}
function setGestureData(data){if(macroDeck)macroDeck.restoreProject(data);else detachedGestures=data;}
function regionProject(selectedOnly=false) {
  const doc=projectDocument(), range=selectedEditRange();
  const ids=new Set(selectedOnly?[state.selected]:doc.layers.map(l=>l.id));
  if(selectedOnly){
    const layers=new Map(doc.layers.map(l=>[l.id,l]));
    for(const id of ids)for(const ref of ['maskA','maskB']){
      const mask=layers.get(id)?.media?.[ref];
      if(layers.has(mask))ids.add(mask);
    }
  }
  const region=transferRegion(doc,range,range,ids,id=>id,'region/');
  return validateProject({...doc,...region,position:range.start},palettes());
}
async function saveLibraryTrack(kind) {
  const name=byId('template-name').value.trim()||selectedLayer()?.name;
  if(kind==='song'&&!state.audioRef?.sha256)throw Error('Open the song to save a song-specific track.');
  await macroDeck?.finishPendingRecording();
  const doc=regionProject(true), key=libraryKey(kind);
  const entries=[...library(kind),{id:newPrefix(),name,doc,rule:wandRule()}];
  localStorage.setItem(key,JSON.stringify(entries));
  libraryCache.set(key,entries);refreshTrackSources();
  byId('track-source').value=kind+':'+entries.at(-1).id;
  status('Saved '+name);
}
function mergeClips(doc,source,placements,label,applyRule=false) {
  const existing=authoredState();
  let next=Math.max(highestId+1,state.nextId);
  const ids=new Map(doc.layers.map(l=>[l.id,next++]));
  const created=new Map(), imported={macros:[],audioGroups:[],lyricGroups:[]};
  for(const placement of placements){
    const region=transferRegion(doc,source,placement,new Set(ids.keys()),id=>ids.get(id),newPrefix());
    for(const l of region.layers){
      if(applyRule)for(const interval of l.envelope){
        const span=interval.sourceEnd-interval.sourceStart;
        interval.level*=placement.level;
        interval.fadeIn=Math.max(interval.fadeIn, Math.min(span,placement.fadeIn));
        interval.fadeOut=Math.max(interval.fadeOut, Math.min(span,placement.fadeOut));
      }
      if(!created.has(l.id))created.set(l.id,l);
      else created.get(l.id).envelope.push(...l.envelope);
    }
    for(const k of Object.keys(imported))imported[k].push(...region.gestures[k]);
  }
  if(!created.size)throw Error('The project contains no tracks in the chosen source.');
  const gestures={...existing.gestures};
  for(const key of Object.keys(imported))gestures[key]=[...gestures[key],...imported[key]];
  gestures.selected=imported.macros[0]?.id??gestures.selected;
  const added=[...created.values()];
  const groups = new Map();
  for (const layer of added) if (layer.presetGroup != null) {
    if (!groups.has(layer.presetGroup)) groups.set(layer.presetGroup,newPrefix());
    layer.presetGroup = groups.get(layer.presetGroup);
  }
  const importedB=!existing.projectorB && added.some(l=>l.projector==='b') ? {projectorB:doc.projectorB} : {};
  const combined={...existing,...importedB,layers:[...added,...existing.layers],nextId:next,selected:ids.get(doc.selected)??added[0].id,gestures};
  validateProject({...projectDocument(),...combined},palettes());
  restoreEdits(combined,false);highestId=next-1;
  checkpoint(label);status(label);
}
async function addTrackFromSource() {
  const sourceValue=byId('track-source').value, colon=sourceValue.indexOf(':');
  const kind=sourceValue.slice(0,colon), id=sourceValue.slice(colon+1);
  const rule=wandRule();
  await macroDeck?.stopForEdit();
  let range=selectedTimeRange();
  if(kind==='shipped'){
    if(!range)throw Error('Select a waveform interval or choose Select all.');
    const s=shippedTracks.get(id), envelope=planIntervals(rule,range,{groups:selectedCues(),analysis:state.analysis});
    if(!envelope.length)throw Error('The boundary found no matches in this selection.');
    const preset = slidePresets.get(s.palette);
    if (preset) {
      addDyeSlide(preset,envelope);
      checkpoint('Add '+s.name);
      return;
    }
    const layer=makeLayer();
    layer.name=s.name;layer.motion=s.motion;layer.palette=s.palette;
    layer.colors=[LIQUID_PALETTES[s.palette][0]];
    layer.nativeLive=nativeDesktop;
    layer.envelope=envelope;
    state.layers.unshift(layer);selectLayer(layer.id);checkpoint('Add '+s.name);status('Added '+s.name);return;
  }
  const entry=library(kind).find(e=>e.id===id);
  if(!entry)throw Error('Choose a saved track or template.');
  const doc=validateProject(entry.doc,palettes()), source=doc.timeline.range;
  await resolveProjectMedia(doc);
  if(kind==='song'&&!range){mergeClips(doc,source,[source],'Restore song track '+entry.name);return;}
  if(!range)throw Error('Select a waveform interval or choose Select all.');
  const placements=planIntervals(rule,range,{groups:selectedCues(),analysis:state.analysis});
  if(!placements.length)throw Error('The boundary found no matches in this selection.');
  mergeClips(doc,source,placements,'Add '+entry.name,true);
}
for(const [button,kind] of [['save-song-track','song'],['save-user-template','user']]){
  byId(button).addEventListener('click',()=>projectOperation('Save track',()=>saveLibraryTrack(kind)));
}
byId('track-source').addEventListener('change',()=>{
  const value=byId('track-source').value, colon=value.indexOf(':');
  if(colon<0)return;
  const kind=value.slice(0,colon),id=value.slice(colon+1);
  if (kind === 'slide') return;
  const entry=kind==='shipped'?shippedTracks.get(id):library(kind).find(e=>e.id===id);
  if(entry?.rule)restoreWandRule({...wandRule(),...entry.rule});
});
async function resolveProjectMedia(doc) {
  const sources=new Map(doc.layers.filter(l=>l.media).map(l=>[l.media.sha256,l.media.name]));
  const offline=[];
  for(const [sha256,name] of sources) {
    if(visualGrants.has(sha256))continue;
    try {
      const grant=desktopInvoke && await desktopInvoke('resolve_visual_media',{sha256,name});
      if(grant?.sha256===sha256) {visualGrants.set(sha256,grant);continue;}
    } catch(e) {offline.push(name+' ('+(e.message||e)+')');continue;}
    offline.push(name);
  }
  return offline;
}
async function openProject(file,merge=false) {
  if (file && file.size > 64 * 1024 * 1024) throw Error('Project exceeds 64 MiB.');
  const choice = file ? [file.name, await file.text(), null] : await desktopInvoke('open_project');
  if (!choice) return;
  const [name, text, selection] = choice;
  const doc = validateProject(JSON.parse(text), palettes());
  await macroDeck?.stopForEdit();
  if(merge) {
    const target=selectedTimeRange();
    if(!target)throw Error('Select the destination time range.');
    const duration=doc.timeline?.duration || (state.audioRef && state.audioRef.sha256===doc.audio?.sha256 ? timelineDuration() : numeric('length'));
    const source=doc.timeline?.range??{start:0,end:duration};
    if(!validRange(source))throw Error('Set the source clip length for this project.');
    await resolveProjectMedia(doc);
    mergeClips(doc,source,[target],'Merge '+name+' into selection');
    return;
  }
  if (!await chooseReplacement()) return;
  const offlineMedia=await resolveProjectMedia(doc);
  const token = ++state.fileToken;
  let audio = null;
  if (selection) {
    const bytes = await desktopInvoke('read_pending_audio', {selectionId:selection[0]});
    audio = await prepareAudio(byteFile(doc.audio.name, bytes), token, doc.audio, selection[0]);
    if (!audio || token !== state.fileToken) throw Error('Audio selection changed.');
  }
  // Identical numeric IDs in two projects must never share a live slide pose.
  if (nativeSurface) {
    const selected = state.selected;
    try {
      state.selected = null;
      await nativeSurface.sync();
      for (const layer of state.layers) retainedNativeLayers.add(layer.id);
      for (const layerId of retainedNativeLayers) await desktopInvoke('forget_live_wheel_layer', {layerId});
      retainedNativeLayers.clear();
    } finally { state.selected = selected; }
  }
  if (selection) await desktopInvoke('confirm_audio', {selectionId:selection[0]});
  else if (desktopInvoke) await desktopInvoke('clear_project_audio');
  pause();
  projectName = name;
  restoreEdits(doc, true);
  installAudio(audio, doc.audio, selection?.[1], selection?.[0], doc.position);
  resetHistory('Open project'); savedEdits = editSignature();
  const audioNotice=doc.audio&&!audio?' Relink '+doc.audio.name+' to play.':'';
  const solverNotice = !solverAvailable && state.layers.some(liveFluidTrack) ? ' The Basilisk solver is unavailable on this installation. Saved fluid seeds are retained.' : '';
  status('Opened '+name+'.'+audioNotice+solverNotice+(offlineMedia.length?' Offline visual media: '+offlineMedia.join(', '):''));
}
async function saveProject(regionOnly=false) {
  await macroDeck?.finishPendingRecording();
  if (state.audioRef && !state.audioRef.sha256) throw Error('This browser could not fingerprint the song. Open the project in the desktop editor or over HTTPS to link audio.');
  const data = regionOnly ? regionProject() : validateProject(projectDocument(), palettes());
  const documentText = JSON.stringify(data);
  const base=projectName?.replace(/\.tss$/i,'') || state.audioName.replace(/\.[^.]+$/, '') || 'session';
  const suggested=base+(regionOnly?'-region':'')+'.tss';
  let name;
  if (desktopInvoke) name = await desktopInvoke('save_project', {document:documentText, suggestedName:suggested});
  else {
    const blob = new Blob([documentText], {type:'application/json'});
    if (blob.size > 64 * 1024 * 1024) throw Error('Project exceeds 64 MiB.');
    const url = URL.createObjectURL(blob), a = document.createElement('a');
    a.href = url; a.download = suggested; a.click();
    setTimeout(() => URL.revokeObjectURL(url), 30000);
    name = suggested;
  }
  if (!name) return;
  if(regionOnly){status('Saved selected region to '+name);return;}
  projectName = name; byId('track-title').textContent = name;
  savedEdits = editSignature(); status('Saved ' + name);
}
byId('project-operation').addEventListener('cancel', event => event.preventDefault());
function pickProject(merge) {
  if(merge && !selectedTimeRange()){status('Select the destination time range.');return;}
  if(desktopInvoke)projectOperation(merge?'Merge project':'Open project',()=>openProject(null,merge));
  else {byId('project-file').dataset.merge=String(merge);byId('project-file').click();}
}
byId('open-project').addEventListener('click',()=>pickProject(false));
byId('merge-region-project').addEventListener('click',()=>pickProject(true));
byId('save-region-project').addEventListener('click',()=>projectOperation('Save region project',()=>saveProject(true)));
byId('project-file').addEventListener('change', event => {
  const file = event.target.files[0]; event.target.value = '';
  const merge=event.target.dataset.merge==='true';
  if (file) projectOperation(merge?'Merge project':'Open project', () => openProject(file,merge));
});
byId('save-project').addEventListener('click', () => projectOperation('Save project', saveProject));
byId('undo-edit').addEventListener('click', () => restoreHistory(editHistory.cursor - 1));
byId('redo-edit').addEventListener('click', () => restoreHistory(editHistory.cursor + 1));
byId('edit-history').addEventListener('change', event => restoreHistory(Number(event.target.value)));
byId('add-layer').addEventListener('click', () => checkpoint('Add visual layer'));
byId('replay-release').addEventListener('click', () => checkpoint('Replay release'));
byId('native-export-baseline').addEventListener('change', () => checkpoint('Set export starting pose'));
byId('wheel-focus-reset').addEventListener('click', () => checkpoint('Restore wheel focus'));
byId('palette-grid').addEventListener('click', () => checkpoint('Set palette'));
byId('layer-list').addEventListener('click', () => checkpoint('Select layer'));
byId('layer-list').addEventListener('keydown', event => {
  if (event.key === 'Enter' || event.key === ' ') checkpoint('Select layer');
});
document.querySelector('.control-body').addEventListener('change', event => {
  if (!event.target.closest('#performer-deck, #artwork-settings')) checkpoint(event.target.getAttribute('aria-label') || event.target.labels?.[0]?.textContent.trim().split('\n')[0] || 'Edit layer');
});
window.addEventListener('resize', drawWaveform);
window.addEventListener('beforeunload', () => proceduralPreview?.dispose());
function addArtwork(media) {
  const layer=makeLayer();
  delete layer.simulationSeed;
  delete layer.filmDepth;
  Object.assign(layer,{name:media.name,media,opacity:1,colors:[...LIQUID_PALETTES.orchid],visibleFilms:'both'});
  state.layers.unshift(layer);
  selectLayer(layer.id);
  checkpoint('Add artwork');
  return layer;
}
visualMedia = nativeDesktop ? installVisualMedia({
  invoke:desktopInvoke, selected:selectedLayer,layers:()=>state.layers,
  isWheel,
  add:addArtwork,
  changed:label=>{renderTimeline();renderControls();checkpoint(label);},report:status,
}) : null;
byId('artwork-add').hidden=!nativeDesktop;
if (!nativeDesktop) addLayer();
renderTimeline(); renderControls(); updateTransport(); requestAnimationFrame(renderFrame);
const presetsReady = loadSlidePresets();
presetsReady.then(() => { if (!editHistory.entries.length) { resetHistory('New session'); savedEdits = editSignature(); } });
