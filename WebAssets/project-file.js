// A project contains authored state. Canvas, Web Audio and native presentation objects
// are rebuilt by the editor when the document is installed.
import {validEnvelope,validRange,glyphs,modes} from './timeline.js';
export const PROJECT_FORMAT = 'the-sacred-swirls';
export const emptyGestures = () => ({macros:[], audioGroups:[], lyricGroups:[], selected:null});

const layerFields = [
  'id', 'name', 'visible', 'palette', 'colors', 'customColors', 'hue', 'speed', 'scale', 'opacity',
  'nativeLive', 'motion', 'motionTake', 'visibleFilms', 'slide', 'gesture', 'tugResponse', 'mousePressure', 'renderWidth'
];

export function layerSettings(layer) {
  const settings=Object.fromEntries(layerFields.map(key => [key, layer[key]]));
  if(layer.envelope!=null) settings.envelope=layer.envelope;
  return settings;
}

const flag = v => typeof v === 'boolean';
const string = v => typeof v === 'string';
const named = v => string(v) && v.length > 0;
const num = (v, low, high = Number.MAX_VALUE) => Number.isFinite(v) && v >= low && v <= high;
const index = v => Number.isSafeInteger(v) && v >= 0;
const id = v => index(v) && v > 0;
const oneOf = (v, ...allowed) => allowed.includes(v);
const record = v => v !== null && typeof v === 'object' && !Array.isArray(v);
const unique = ids => new Set(ids).size === ids.length;
const slide = v => oneOf(v, 'front', 'rear');
const gesture = v => oneOf(v, 'tug', 'pressure', 'rotate');
const pair = v => record(v) && num(v.front, 0, 1) && num(v.rear, 0, 1);
const requireField = (valid, field) => { if (!valid) throw Error('Invalid project field: ' + field); };

function validLayer(l, palettes) {
  return record(l) && validEnvelope(l.envelope) && id(l.id) && named(l.name) && flag(l.visible) && palettes.has(l.palette) &&
    Array.isArray(l.colors) && l.colors.length === 4 && l.colors.every(c => string(c) && /^#[0-9a-f]{6}$/i.test(c)) &&
    flag(l.customColors) && num(l.hue, 0, 360) && num(l.speed, 0, 2) && num(l.scale, .4, 2) && num(l.opacity, 0, 1) &&
    flag(l.nativeLive) && oneOf(l.motion, 'wheel-flow', 'coral-release', 'wheel-with-coral-beads') &&
    index(l.motionTake) && oneOf(l.visibleFilms, 'both', 'front', 'rear', 'clear') && slide(l.slide) &&
    gesture(l.gesture) && pair(l.tugResponse) && pair(l.mousePressure) && oneOf(l.renderWidth, 640, 960);
}

function validMacro(m) {
  if (!(record(m) && named(m.id) && named(m.name) && num(m.duration, 0) && m.duration>0 &&
        (m.songStart === null || num(m.songStart, 0)) && flag(m.enabled) && flag(m.fit) &&
        num(m.amount, 0, 3) && num(m.step, 0, 3) && num(m.cap, 0, 4) && string(m.cue) &&
        Array.isArray(m.muted) && m.muted.every(index) && unique(m.muted) && Array.isArray(m.events) && m.events.length)) return false;
  let previous = 0;
  for (const e of m.events) {
    if (!(record(e) && num(e.t, previous, m.duration) && id(e.layerId) && e.layerId === m.events[0].layerId &&
          slide(e.slide) && gesture(e.gesture) && num(e.tugResponse, 0, 1) && oneOf(e.phase, 'down', 'move', 'up') &&
          Array.isArray(e.point) && e.point.length === 2 && e.point.every(p => num(p, 0, 1)) &&
          (e.gesture !== 'pressure' || num(e.amount, 0, 1)))) return false;
    previous = e.t;
  }
  return true;
}

function validGroup(g, kind) {
  return record(g) && named(g.id) && named(g.label) && g.kind === kind && Array.isArray(g.occurrences) &&
    g.occurrences.every(h => record(h) && num(h.start, 0) && num(h.end, 0) && h.end > h.start &&
      (h.confidence === undefined || num(h.confidence, 0)));
}

export function validateProject(p, palettes) {
  requireField(record(p) && p.format === PROJECT_FORMAT, 'format');
  if (p.version !== 1) throw Error('Unsupported TSS project version: ' + p.version);
  requireField(p.audio === null || (record(p.audio) && named(p.audio.name) && !/[\\/\u0000]/.test(p.audio.name) &&
    /^[0-9a-f]{64}$/.test(p.audio.sha256)), 'audio');
  requireField(num(p.position, 0), 'position');
  if(p.timeline !== undefined) {
    const t=p.timeline;
    requireField(record(t) && num(t.duration,0) && (t.range===null || (validRange(t.range) && t.range.end<=t.duration)) &&
      record(t.bindings) && glyphs.every(g=>modes.includes(t.bindings[g])), 'timeline');
  }
  requireField(Array.isArray(p.layers), 'layers');
  p.layers.forEach((layer, i) => requireField(validLayer(layer, palettes), 'layers[' + i + ']'));
  const ids = p.layers.map(l => l.id);
  requireField(unique(ids) && (ids.length ? ids.includes(p.selected) : p.selected === null), 'layer selection');
  requireField(id(p.nextId) && ids.every(value => value < p.nextId), 'nextId');
  requireField(p.focus === null || num(p.focus, 0, 1), 'focus');
  requireField(oneOf(p.exportBaseline, 'auto', 'current', 'neutral'), 'exportBaseline');
  const f = p.feedback;
  requireField(record(f) && flag(f.enabled) && num(f.delay, 25, 450) && num(f.strength, 0, 95) &&
    num(f.zoom, 85, 115) && num(f.rotation, -12, 12), 'feedback');
  const g = p.gestures;
  requireField(record(g) && Array.isArray(g.macros) && g.macros.every(validMacro) &&
    unique(g.macros.map(m => m.id)), 'gesture macros');
  requireField(g.selected === null || g.macros.some(m => m.id === g.selected), 'selected macro');
  requireField(Array.isArray(g.audioGroups) && g.audioGroups.every(c => validGroup(c, 'audio')) &&
    Array.isArray(g.lyricGroups) && g.lyricGroups.every(c => validGroup(c, 'lyric')) &&
    unique([...g.audioGroups, ...g.lyricGroups].map(c => c.id)), 'cue groups');
  return p;
}

// History entries are immutable authored snapshots. Consecutive unchanged commands
// do not introduce steps. Branching from an earlier edit discards its redo tail.
export class EditHistory {
  constructor() { this.entries = []; this.cursor = -1; }
  reset(state, label) {
    this.entries = []; this.cursor = -1;
    this.record(state, label);
  }
  record(state, label) {
    const previous = this.entries[this.cursor]?.fields;
    const fields = {};
    let changed = false;
    for (const [key, value] of Object.entries(state)) {
      const json = JSON.stringify(value);
      if (previous?.[key] === json) fields[key] = previous[key];
      else { fields[key] = json; changed = true; }
    }
    if (!changed) return;
    this.entries.splice(this.cursor + 1);
    this.entries.push({fields, label});
    // Count shared cue and gesture snapshots once across visual edits.
    const chunks = new Set();
    let chars = 0;
    for (let i = this.entries.length - 1; i >= 0; --i) {
      for (const json of Object.values(this.entries[i].fields)) {
        if (!chunks.has(json)) { chunks.add(json); chars += json.length; }
      }
      if (i < this.entries.length - 1 && (chars > 32 * 1024 * 1024 || this.entries.length - i > 128)) {
        this.entries.splice(0, i + 1);
        break;
      }
    }
    this.cursor = this.entries.length - 1;
  }
  stateAt(cursor) {
    if (!Number.isInteger(cursor) || cursor < 0 || cursor >= this.entries.length) return null;
    return Object.fromEntries(Object.entries(this.entries[cursor].fields).map(([key, json]) => [key, JSON.parse(json)]));
  }
}
