import test from 'node:test';
import assert from 'node:assert/strict';
import { reviewTranscript, transcriptVtt } from '../../WebAssets/local-lyrics.js';

test('review preserves distinct occurrences, literal text and clipped final model timing', () => {
  const vtt = transcriptVtt([
    {start:0,end:1.02,text:'Turn the glass slowly.'},
    {start:2.04,end:4.32,text:'Blue < violet & green.'},
    {start:5.38,end:6.48,text:'Turn the glass slowly.'},
    {start:7,end:8,text:'trailing model output'},
  ], 6.38);
  const review = reviewTranscript(vtt, 6.38);
  assert.equal(review.count, 3);
  assert.match(review.text, /Blue &lt; violet &amp; green/);
  assert.deepEqual(review.groups[0].occurrences.map(o=>[o.start,o.end]), [[0,1.02],[5.38,6.38]]);
});

test('partially edited or out-of-song timing cannot replace the current cue set', () => {
  for (const line of ['broken', '00:00:04.000 --> 00:00:02.000', '00:00:00.010 --> 00:00:05.000']) {
    assert.throws(()=>reviewTranscript('WEBVTT\n\n'+line+'\nHold the glass.',4));
  }
  assert.throws(()=>reviewTranscript('WEBVTT\n\n00:00:00.000 --> 00:00:03.000\n',4));
});

test('changing recordings cancels the outstanding analysis and discards a late result', async () => {
  const { installLocalLyrics } = await import('../../WebAssets/local-lyrics.js');
  const elements = new Map();
  const original = globalThis.document;
  globalThis.document = { getElementById(id) {
    if (!elements.has(id)) elements.set(id, {value:'', addEventListener(name, f) { this[name] = f; }});
    return elements.get(id);
  }};
  try {
    let finish;
    const calls = [], applied = [];
    const invoke = (command, args) => {
      calls.push([command, args]);
      return command === 'transcribe_song' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(true);
    };
    const deck = installLocalLyrics({invoke,apply: g => applied.push(g)});
    const get = id => elements.get('lyrics-' + id);
    deck.source('/one.m4a', {duration:5});
    const first = get('run').click();
    deck.source('/two.mp3', {duration:6});
    finish([{start:0,end:1,text:'First song'}]);
    await first;
    assert.equal(get('draft').value,'');
    assert.equal(calls[1][0],'cancel_transcription');
    const next = get('run').click();
    finish([{start:0,end:1,text:'Return now'},{start:3,end:4,text:'Return now'}]);
    await next;
    assert.equal(applied.length,0);
    get('apply').click();
    assert.equal(applied[0][0].occurrences.length,2);
    await get('save').click();
    assert.equal(calls.at(-1)[1].audioPath,'/two.mp3');
  } finally { globalThis.document = original; }
});
