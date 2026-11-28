import {equipmentProfiles} from './equipment.js';

export const MAX_PACK_BYTES = 256 * 1024;
const storagePrefix = 'tss.legends.v1/';
const reviewStatuses = new Set(['original', 'research', 'artist-contributed', 'artist-reviewed']);

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}
function objectWithFields(value, fields, name) {
  requireValue(value && typeof value === 'object' && !Array.isArray(value), name+' must be an object.');
  const keys = Object.keys(value);
  requireValue(keys.length === fields.length && fields.every(key => Object.hasOwn(value, key)),
    name+' must contain exactly '+fields.join(', ')+'.');
}
function textField(value, name, max) {
  requireValue(typeof value === 'string' && value.trim() && value.length <= max, name+' must contain 1–'+max+' characters.');
}
function identifier(value, name) {
  requireValue(typeof value === 'string' && value.length <= 64 && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value),
    name+' must use lowercase letters, digits and single hyphens (up to 64 characters).');
}
function list(value, name, min, max) {
  requireValue(Array.isArray(value) && value.length >= min && value.length <= max,
    name+' must contain '+min+'–'+max+' items.');
}
function validateSource(source) {
  list(source, 'Each source', 2, 2);
  textField(source[0], 'Source label', 160);
  textField(source[1], 'Source URL', 2048);
  let url;
  try { url = new URL(source[1]); } catch { /* Report the field below. */ }
  requireValue(url?.protocol === 'https:' && /^https:\/\//i.test(source[1]) && !url.username && !url.password &&
    !/[\s\u0000-\u001f\u007f\\]/u.test(source[1]), 'Source URL must be an HTTPS link without credentials.');
}
function inRange(value, name, min, max) {
  requireValue(Number.isFinite(value) && value >= min && value <= max, name+' must be in the range '+min+'–'+max+'.');
}
function validateRig(entry) {
  objectWithFields(entry, ['id','artist','venue','record','sources','setup','rig'], 'Rig entry');
  identifier(entry.id, 'Rig ID');
  textField(entry.artist, 'Artist or studio', 160);
  textField(entry.venue, 'Venue or setting', 200);
  textField(entry.record, 'Record', 2400);
  textField(entry.setup, 'Setup', 2400);
  list(entry.sources, 'Sources', 0, 16);
  entry.sources.forEach(validateSource);
  const rig = entry.rig;
  objectWithFields(rig, ['a','b','temperatureA','temperatureB','levelB','offsetX','focusA','focusB'], 'Projector settings');
  for (const key of ['a','b']) {
    requireValue(typeof rig[key] === 'string' && Object.hasOwn(equipmentProfiles, rig[key]), 'Unknown projector '+key.toUpperCase()+' profile.');
  }
  for (const key of ['temperatureA','temperatureB']) {
    requireValue(Number.isInteger(rig[key]), key+' must be a whole number of kelvin.');
    inRange(rig[key], key, 2700, 10000);
  }
  for (const key of ['levelB','focusA','focusB']) inRange(rig[key], key, 0, 1);
  inRange(rig.offsetX, 'offsetX', -1, 1);
}
function freeze(value) {
  if (value && typeof value === 'object') {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}

export function parseLegendPack(text) {
  requireValue(typeof text === 'string' && text.length <= MAX_PACK_BYTES &&
    new TextEncoder().encode(text).length <= MAX_PACK_BYTES, 'Legends pack exceeds the 256 KiB limit or is unreadable.');
  let pack;
  try { pack = JSON.parse(text); } catch { throw new Error('Legends pack must be a JSON document.'); }
  requireValue(pack?.format === 'tss.legends' && pack.schemaVersion === 1, 'Unsupported Legends pack format or schema version.');
  objectWithFields(pack, ['format','schemaVersion','id','title','version','credits','review','rigs'], 'Legends pack');
  identifier(pack.id, 'Pack ID');
  textField(pack.title, 'Pack title', 160);
  requireValue(typeof pack.version === 'string' && pack.version.length <= 32 &&
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(pack.version), 'Pack version must use three numbers (e.g. 1.0.0).');
  list(pack.credits, 'Credits', 1, 16);
  pack.credits.forEach(credit => textField(credit, 'Credit', 200));
  objectWithFields(pack.review, ['status','note'], 'Pack review');
  requireValue(reviewStatuses.has(pack.review.status), 'Unknown pack review status.');
  textField(pack.review.note, 'Review note', 1200);
  list(pack.rigs, 'Rigs', 1, 32);
  pack.rigs.forEach(validateRig);
  requireValue(new Set(pack.rigs.map(rig => rig.id)).size === pack.rigs.length, 'Rig IDs must be unique in a pack.');
  return freeze(pack);
}

function storedEntry(text, id) {
  requireValue(typeof text === 'string' && text.length <= MAX_PACK_BYTES+128, 'Invalid stored Legends pack.');
  const item = JSON.parse(text);
  const legacy = item?.format === 'tss.legends';
  requireValue(legacy || typeof item?.enabled === 'boolean', 'Invalid stored Legends pack setting.');
  const pack = parseLegendPack(legacy ? text : JSON.stringify(item.pack));
  requireValue(pack.id === id, 'Stored ID differs from the pack ID.');
  return {enabled:legacy || item.enabled, pack};
}

// One atomic Storage item per pack keeps a replacement and its enable switch together.
export function createLegendLibrary(storage) {
  const installed = new Map();
  const unreadable = new Set();
  let error = '';
  try {
    storage ??= globalThis.localStorage;
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (!key?.startsWith(storagePrefix)) continue;
      try {
        const entry = storedEntry(storage.getItem(key), key.slice(storagePrefix.length));
        installed.set(entry.pack.id, entry);
      } catch {
        unreadable.add(key);
      }
    }
  } catch {
    error = 'Legends pack storage is unavailable.';
    storage = null;
  }
  function requireStorage() { requireValue(storage, 'Legends pack storage is unavailable.'); }
  function save(pack, enabled) {
    const entry = {enabled, pack};
    storage.setItem(storagePrefix+pack.id, JSON.stringify(entry));
    installed.set(pack.id, entry);
    unreadable.delete(storagePrefix+pack.id);
  }
  return {
    get error() { return error || (unreadable.size ? 'An installed Legends pack could not be loaded. Import its file again to restore it.' : ''); },
    get packs() { return Object.freeze(Array.from(installed.values(), entry => entry.pack)); },
    isEnabled(id) { return installed.get(id)?.enabled ?? false; },
    setEnabled(id, enabled) {
      identifier(id, 'Pack ID');
      requireValue(typeof enabled === 'boolean', 'Pack availability must be true or false.');
      requireStorage();
      const existing = storage.getItem(storagePrefix+id);
      requireValue(existing !== null, 'Legends pack is not installed.');
      const {pack} = storedEntry(existing, id);
      save(pack, enabled);
    },
    install(text, {replace = false} = {}) {
      const pack = parseLegendPack(text);
      requireStorage();
      const key = storagePrefix+pack.id;
      const existing = storage.getItem(key);
      let previous;
      if (existing !== null) {
        try { previous = storedEntry(existing, pack.id); }
        catch (cause) { if (!unreadable.has(key)) throw cause; }
      }
      requireValue(replace || !previous, 'This pack ID is already installed.');
      save(pack, previous?.enabled ?? true);
      return pack;
    },
    remove(id) {
      requireStorage();
      identifier(id, 'Pack ID');
      const key = storagePrefix+id;
      const existed = storage.getItem(key) !== null;
      storage.removeItem(key);
      installed.delete(id);
      unreadable.delete(key);
      return existed;
    }
  };
}
