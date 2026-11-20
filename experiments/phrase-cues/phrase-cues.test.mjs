import assert from 'node:assert/strict';
import test from 'node:test';
import { audioCueGroups, lyricCueGroups } from '../../WebAssets/phrase-cues.js';

const frames = n => Array.from({ length: n }, () => ({ bass: 0, mid: 0, treble: 0, energy: 0 }));
function emit(data, ticks, names, strength = .7) {
  ticks.forEach((start, n) => {
    for (let a = 0; a < 4; a++) {
      const v = strength * [.8, .65, .40, .22][a];
      data[start + a][names[n]] = v;
      data[start + a].energy = v;
    }
  });
}
const ranges = group => group.occurrences.map(o => [o.start, o.end]);
test('follows registered 30 Hz band patterns through gain, jitter, and moderate tempo variation', () => {
  const data = frames(250);
  const timbre = ['bass', 'treble', 'mid', 'treble'];
  emit(data, [12, 23, 40, 48], timbre);
  emit(data, [110, 121, 141, 149], timbre, .31);
  emit(data, [185, 200, 213, 235], ['mid', 'mid', 'bass', 'bass'], .48);
  const groups = audioCueGroups({ fps: 30, frames: data });
  assert.equal(groups.length, 1);
  assert.equal(groups[0].kind, 'audio');
  assert.deepEqual(ranges(groups[0]), [[12 / 30, 49 / 30], [110 / 30, 150 / 30]]);
  assert.ok(groups[0].occurrences.every(o => o.confidence > 0 && o.confidence <= 1));
  assert.deepEqual(audioCueGroups({ fps: 30, frames: data }), groups);
});

test('silence, sustained energy, progressions without a repeat and overlapping windows give no motif', () => {
  const none = frames(100);
  assert.deepEqual(audioCueGroups({ fps: 30, frames: none }), []);
  assert.deepEqual(audioCueGroups({ fps: 30, frames: none.map(() => ({ bass: .5 })) }), []);
  emit(none, [6, 14, 22, 30, 38, 46], Array(6).fill('bass'));
  assert.deepEqual(audioCueGroups({ fps: 30, frames: none }), []);
  const diff = frames(195);
  emit(diff, [1, 10, 35, 43], ['bass', 'mid', 'treble', 'mid']);
  emit(diff, [100, 123, 128, 144], ['bass', 'mid', 'treble', 'mid']);
  assert.deepEqual(audioCueGroups({ fps: 30, frames: diff }), []);
});

test('top-level local-transcribe JSON preserves real segment boundaries and punctuation-insensitive recurrences', () => {
  const raw = [{ start: 1, end: 3, text: 'Keep the river rolling!' },
    { start: 6, end: 7, text: 'One bridge' },
    { start: 10.5, end: 13, text: 'keep THE river rolling…' }];
  const found = lyricCueGroups(JSON.stringify(raw), 'song.json');
  assert.equal(found.length, 1);
  assert.equal(found[0].kind, 'lyric');
  assert.deepEqual(ranges(found[0]), [[1, 3], [10.5, 13]]);
  assert.equal(found[0].label, raw[0].text);
});

test('partial phrases in distinct local-transcribe segments keep enclosing cue times', () => {
  const raw = [{ start: 1, end: 3, text: 'Now see the river' }, { start: 5, end: 8, text: 'See the river home' }];
  const found = lyricCueGroups(JSON.stringify(raw), 'song.json');
  assert.deepEqual(found.map(r => r.id), ['lyric-see the river']);
  assert.deepEqual(ranges(found[0]), [[1, 3], [5, 8]]);
});

test('real word timing locates shared words inside different lines and across ASR segments', () => {
  const tokens = [['blue',1], ['paper',1.4], ['moon',1.8], ['again',4], ['blue',8], ['paper',8.3], ['moon',8.7]];
  const words = tokens.map(([word, start]) => ({ word, start, end: start + .25 }));
  const nested = { segments: [{ words: words.slice(0, 2) }, { words: words.slice(2, 4) }, { words: words.slice(4) }] };
  const result = lyricCueGroups(JSON.stringify(nested), 'timed.json');
  assert.deepEqual(result.map(x => x.id), ['lyric-blue paper moon']);
  assert.deepEqual(ranges(result[0]), [[1, 2.05], [8, 8.95]]);
  assert.deepEqual(lyricCueGroups(JSON.stringify({ words })), result);
});

test('WebVTT and SRT handle settings, style tags, comma timestamps and stage directions', () => {
  const vtt = 'WEBVTT\n\nNOTE marker\n00:00.000 --> 00:01.000\nignored\n\na\n00:01.250 --> 00:02.750 align:start\n<v Solo><i>Let’s</i> go</v>\n\n00:03.500 --> 00:05.000\n\u266b let\'s go!\n\n00:06.000 --> 00:07.000\n[music]';
  const srt = "1\r\n00:00:01,250 --> 00:00:02,750\r\nLet's go\r\n\r\n2\r\n00:00:03,500 --> 00:00:05,000\r\nLet's go!";
  const a = lyricCueGroups(vtt, 'song.vtt'), b = lyricCueGroups(srt, 'song.srt');
  assert.deepEqual(a.map(g => g.id), ["lyric-let's go"]);
  assert.deepEqual(ranges(a[0]), [[1.25, 2.75], [3.5, 5]]);
  assert.deepEqual(ranges(a[0]), ranges(b[0]));
});

test('omits untimed/invalid lines, duplicate observations, unseparated repeats, and lone occurrences', () => {
  assert.deepEqual(lyricCueGroups('a song with no timestamps'), []);
  assert.deepEqual(lyricCueGroups(JSON.stringify([{start:2,end:1,text:'bad'},{start:3,end:4,text:'one lonely line'}])), []);
  assert.deepEqual(lyricCueGroups(JSON.stringify([{start:1,end:4,text:'Never say never say never'},{start:1,end:4,text:'Never say never say never'}])), []);
  assert.throws(() => lyricCueGroups('{"invalid":true}', 'a.json'), /Expected timed segments/);
});

test('normal editor PCM analysis feeds repeat detection without hand-authored feature frames', async () => {
  const { analyzeAudio } = await import('../../WebAssets/audio-features.js');
  const sampleRate = 24000, duration = 7;
  const pcm = new Float32Array(sampleRate * duration);
  const offsets = [0, .4, .95, 1.35], hz = [80, 8500, 850, 8500];
  for (const [origin, gain] of [[.5, .5], [4, .3]]) for (let hit = 0; hit < 4; hit++) {
    const start = Math.round((origin + offsets[hit]) * sampleRate);
    for (let n = 0; n < .11 * sampleRate; n++) {
      const t = n / sampleRate;
      pcm[start + n] += gain * Math.exp(-28 * t) * Math.sin(2 * Math.PI * hz[hit] * t);
    }
  }
  const audio = { sampleRate, duration, length: pcm.length, numberOfChannels: 1, getChannelData: () => pcm };
  const groups = audioCueGroups(await analyzeAudio(audio));
  assert.equal(groups.length, 1);
  assert.match(groups[0].label, /Low · High · Mid · High/);
  assert.ok(Math.abs(groups[0].occurrences[0].start - .5) <= 1 / 30);
  assert.ok(Math.abs(groups[0].occurrences[1].start - 4) <= 1 / 30);
});
