import { lyricCueGroups } from './phrase-cues.js';

function timestamp(seconds) {
  const ms = Math.round(seconds * 1000);
  const pad = n => String(n).padStart(2, '0');
  return pad(Math.floor(ms / 3600000)) + ':' + pad(Math.floor(ms / 60000) % 60) + ':' +
    pad(Math.floor(ms / 1000) % 60) + '.' + String(ms % 1000).padStart(3, '0');
}

export function transcriptVtt(segments, duration) {
  const cues = [];
  for (const { start, end, text } of segments) {
    const until = Math.min(end, duration);
    if (Math.round(until * 1000) <= Math.round(start * 1000)) continue;
    const plain = text.trim().replace(/\s+/gu, ' ').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    cues.push(timestamp(start) + ' --> ' + timestamp(until) + '\n' + plain);
  }
  return 'WEBVTT\n\n' + cues.join('\n\n') + '\n';
}

// The editable draft contains WebVTT cues. Reject malformed edits before replacing existing triggers.
export function reviewTranscript(draft, duration) {
  const text = draft.replace(/\r\n?/g, '\n').trim();
  const [header, ...blocks] = text.split(/\n[ \t]*\n+/);
  if (header !== 'WEBVTT' || !blocks.length) throw new Error('Keep the WEBVTT header and at least one timed cue.');
  const stamp = /^(\d{2,}):([0-5]\d):([0-5]\d)\.(\d{3})$/;
  function seconds(s) {
    const m = stamp.exec(s);
    return m ? +m[1] * 3600 + +m[2] * 60 + +m[3] + +m[4] / 1000 : NaN;
  }
  const segments = [];
  for (let i = 0; i < blocks.length; i++) {
    const [timing, ...lines] = blocks[i].split('\n');
    const [from, to, ...extra] = timing.trim().split(/\s+-->\s+/);
    const start = seconds(from), end = seconds(to);
    if (extra.length || !Number.isFinite(start) || !Number.isFinite(end) || end <= start || end > duration + .0005 ||
        !lines.join(' ').trim() || lines.some(line => line.includes('-->'))) {
      throw new Error('Check the time range and text of cue ' + (i + 1) + '.');
    }
    segments.push({start, end, text:lines.join(' ')});
  }
  return { segments, text: text + '\n', groups: lyricCueGroups(text, 'lyrics.vtt'), count: blocks.length };
}

export function lyricExcerpt(draft, duration, {start,end}) {
  return reviewTranscript(draft,duration).segments.filter(c=>c.start < end && c.end > start)
    .map(c=>c.text.replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/&amp;/g,'&')).join(' ');
}

export function installLocalLyrics({ invoke, apply }) {
  const el = id => document.getElementById('lyrics-' + id);
  const run = el('run'), cancel = el('cancel'), save = el('save'), use = el('apply');
  const draft = el('draft'), review = el('review'), status = el('status');
  let path = null, buffer = null, active = false, generation = 0;
  el('deck').hidden = false;
  use.hidden = !apply;
  function controls() {
    run.disabled = !path || active;
    cancel.hidden = !active;
    use.disabled = save.disabled = active || !draft.value;
  }
  async function stop() {
    try { await invoke('cancel_transcription'); }
    catch (e) { status.textContent = String(e); }
  }
  run.addEventListener('click', async () => {
    if (!path || active) return;
    const id = generation;
    active = true;
    controls();
    status.textContent = 'Transcribing the song locally…';
    try {
      const segments = await invoke('transcribe_song', { audioPath: path });
      if (id !== generation) return;
      const text = transcriptVtt(segments, buffer.duration);
      const result = reviewTranscript(text, buffer.duration);
      draft.value = text;
      review.hidden = false;
      status.textContent = result.count + ' timed cues to review.';
    } catch (e) { if (id === generation) status.textContent = String(e); }
    finally { active = false; controls(); }
  });
  cancel.addEventListener('click', stop);
  draft.addEventListener('input', controls);
  use.addEventListener('click', () => {
    try {
      const result = reviewTranscript(draft.value, buffer.duration);
      apply(result.groups);
      status.textContent = result.groups.length + ' recurring lyric cues applied to this song.';
    } catch (e) { status.textContent = e.message; }
  });
  save.addEventListener('click', async () => {
    const id = generation;
    try {
      const { text } = reviewTranscript(draft.value, buffer.duration);
      const saved = await invoke('save_reviewed_lyrics', { text, audioPath: path });
      if (saved && id === generation) status.textContent = 'Timed lyrics saved.';
    } catch (e) { if (id === generation) status.textContent = String(e); }
  });
  return {
    excerpt(range) {
      return buffer && draft.value ? lyricExcerpt(draft.value, buffer.duration, range) : '';
    },
    source(nextPath, nextBuffer) {
      if (nextPath === path && nextBuffer === buffer) return;
      generation++;
      if (active) void stop();
      path = nextPath;
      buffer = nextBuffer;
      review.hidden = true;
      draft.value = '';
      status.textContent = '';
      controls();
    },
  };
}
