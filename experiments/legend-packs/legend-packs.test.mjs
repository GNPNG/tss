import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {MAX_PACK_BYTES, createLegendLibrary, parseLegendPack} from '../../WebAssets/legend-packs.js';

const example = readFileSync(new URL('../../examples/legend-packs/two-beam-studio.tsslegends.json', import.meta.url), 'utf8');
const edit = change => {
  const pack = JSON.parse(example);
  change(pack);
  return JSON.stringify(pack);
};
class MemoryStorage {
  items = new Map();
  get length() { return this.items.size; }
  key(index) { return [...this.items.keys()][index] ?? null; }
  getItem(key) { return this.items.get(key) ?? null; }
  setItem(key, value) { this.items.set(key, value); }
  removeItem(key) { this.items.delete(key); }
}

test('fresh library is empty; install, reopen, explicitly replace and remove an original pack', () => {
  const storage = new MemoryStorage(), initial = createLegendLibrary(storage);
  assert.deepEqual(initial.packs, []);
  const demo = initial.install(example);
  assert.equal(demo.review.status, 'original');
  assert.deepEqual(createLegendLibrary(storage).packs, [demo]);
  const revision = edit(pack => { pack.version = '1.1.0'; pack.rigs[0].rig.focusA = 0.2; });
  assert.throws(() => initial.install(revision), /already installed/);
  assert.deepEqual(initial.packs, [demo]);
  assert.equal(initial.install(revision, {replace:true}).version, '1.1.0');
  assert.equal(createLegendLibrary(storage).packs[0].rigs[0].rig.focusA, 0.2);
  assert.equal(initial.remove(demo.id), true);
  assert.equal(initial.remove(demo.id), false);
  assert.deepEqual(createLegendLibrary(storage).packs, []);
});

test('independent packs may reuse a rig ID and each pack may supply many rigs', () => {
  const storage = new MemoryStorage();
  const library = createLegendLibrary(storage);
  library.install(example);
  library.install(edit(pack => {
    pack.id = 'second-studio';
    pack.review = {status:'artist-contributed', note:'Settings supplied by the named studio.'};
    pack.rigs.push({...pack.rigs[0], id:'wide-stage'});
  }));
  library.install(edit(pack => { pack.id = 'third-studio'; }));
  assert.deepEqual(createLegendLibrary(storage).packs.map(p => p.rigs.length), [1,2,1]);
  const saved = library.packs[1];
  assert.throws(() => { saved.rigs[1].rig.levelB = 0.9; }, TypeError);
  library.remove('second-studio');
  assert.deepEqual(createLegendLibrary(storage).packs.map(p => p.id), ['two-beam-studio','third-studio']);
});

test('accepts documented limits and rejects malformed metadata, unsupported equipment and invalid links', () => {
  const valid = edit(pack => {
    pack.credits = ['Studio member, photographer'];
    pack.review = {status:'artist-reviewed', note:'Studio supplied this review note.'};
    pack.rigs[0].sources = [['Reference','https://example.org/scans/001?view=large#fig-1']];
    Object.assign(pack.rigs[0].rig, {a:'carousel-550r', b:'film', temperatureA:2700, temperatureB:10000,
      levelB:0, focusA:1, focusB:0, offsetX:-1});
  });
  assert.equal(parseLegendPack(valid).review.status, 'artist-reviewed');
  const bad = [
    pack => pack.schemaVersion++,
    pack => { pack.id = 'Studio One'; },
    pack => { pack.version = 'draft'; },
    pack => { pack.rigs = []; },
    pack => { pack.credits = []; },
    pack => { pack.review = {status:'officially-verified', note:'example'}; },
    pack => { pack.rigs.push(pack.rigs[0]); },
    pack => { pack.rigs[0].rig.b = 'unlisted'; },
    pack => { pack.rigs[0].rig.offsetX = 1.1; },
    pack => { pack.rigs[0].rig.temperatureA = 5400.5; },
    pack => { pack.rigs[0].rig.focusB = '0.5'; },
    pack => { pack.rigs[0].sources = [['bad','file:///tmp/film']]; },
    pack => { pack.rigs[0].sources = [['bad','https://name@example.org/a']]; },
    pack => { pack.extraField = true; },
  ];
  for (const change of bad) assert.throws(() => parseLegendPack(edit(change)));
  assert.throws(() => parseLegendPack('['));
  assert.throws(() => parseLegendPack(' '.repeat(MAX_PACK_BYTES)+'{}'), /256 KiB/);
  assert.throws(() => parseLegendPack('é'.repeat(MAX_PACK_BYTES/2+1)), /256 KiB/);
});

test('validation and storage failures preserve active and installed entries', () => {
  const storage = new MemoryStorage(), library = createLegendLibrary(storage);
  const existing = library.install(example);
  assert.throws(() => library.install(edit(pack => { pack.rigs[0].rig.focusB = 1.2; }), {replace:true}));
  storage.setItem = () => { throw new Error('Device storage full'); };
  assert.throws(() => library.install(edit(pack => {pack.version = '2.0.0';}), {replace:true}), /storage full/);
  assert.deepEqual(library.packs, [existing]);
  assert.deepEqual(createLegendLibrary(storage).packs, [existing]);
  storage.items.set('tss.legends.v1/damaged', '{');
  storage.items.set('unrelated', 'a different application setting');
  const reopened = createLegendLibrary(storage);
  assert.deepEqual(reopened.packs, [existing]);
  assert.match(reopened.error, /could not be loaded/);
  reopened.remove(existing.id);
  assert.equal(storage.getItem('unrelated'), 'a different application setting');
});

test('a failed storage read permits opening the gallery with an error', () => {
  const library = createLegendLibrary({get length() { throw new Error('Unavailable'); }});
  assert.deepEqual(library.packs, []);
  assert.match(library.error, /unavailable/);
  assert.throws(() => library.install(example), /unavailable/);
});

test('independent library handles preserve one another’s pack files', () => {
  const storage = new MemoryStorage(), left = createLegendLibrary(storage), right = createLegendLibrary(storage);
  left.install(example);
  right.install(edit(pack => { pack.id = 'other-studio'; }));
  assert.throws(() => right.install(example), /already installed/);
  left.remove('two-beam-studio');
  assert.deepEqual(createLegendLibrary(storage).packs.map(p => p.id), ['other-studio']);
});

test('disabled packs stay installed and toggles survive reload, unrelated changes and replacement', () => {
  const storage = new MemoryStorage(), library = createLegendLibrary(storage);
  const original = library.install(example);
  assert.equal(library.isEnabled(original.id), true);
  library.setEnabled(original.id, false);
  const reopened = createLegendLibrary(storage);
  assert.deepEqual(reopened.packs, [original]);
  assert.equal(reopened.isEnabled(original.id), false);
  const revised = reopened.install(edit(pack => { pack.version = '2.0.0'; }), {replace:true});
  reopened.install(edit(pack => { pack.id = 'third-studio'; }));
  assert.deepEqual(reopened.packs[0], revised);
  assert.equal(reopened.isEnabled(original.id), false);
  assert.equal(reopened.isEnabled('third-studio'), true);
  library.setEnabled(original.id, true);
  const enabled = createLegendLibrary(storage);
  assert.equal(enabled.isEnabled(original.id), true);
  assert.equal(enabled.packs[0].version, '2.0.0');
  enabled.remove(original.id);
  assert.equal(enabled.isEnabled(original.id), false);
  assert.throws(() => enabled.setEnabled(original.id, true), /not installed/);
});

test('both source research packs retain independent provenance and can be disabled separately', () => {
  const library = createLegendLibrary(new MemoryStorage());
  for (const id of ['bill-ham','joshua-light-show']) {
    const source = new URL('../../legend-packs/research/'+id+'.tsslegends.json', import.meta.url);
    const pack = library.install(readFileSync(source, 'utf8'));
    assert.equal(pack.id, id);
    assert.equal(pack.review.status, 'research');
    assert.ok(pack.rigs[0].sources.length > 0);
  }
  library.setEnabled('bill-ham', false);
  assert.equal(library.isEnabled('joshua-light-show'), true);
});

test('reimport restores a damaged item identified on startup', () => {
  const storage = new MemoryStorage();
  storage.items.set('tss.legends.v1/two-beam-studio', '{');
  const library = createLegendLibrary(storage);
  assert.ok(library.error);
  assert.equal(library.install(example).id, 'two-beam-studio');
  assert.equal(library.error, '');
  assert.equal(createLegendLibrary(storage).packs.length, 1);
});
