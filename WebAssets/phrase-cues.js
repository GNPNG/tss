// Source times are seconds. Both detectors return repeated, non-overlapping observations.
const bands = ['bass', 'mid', 'treble'];
const bandLabels = ['Low', 'Mid', 'High'];

function nonOverlapping(items) {
  const ordered = items.slice().sort((a, b) => a.start - b.start || a.end - b.end);
  const found = [];
  for (const item of ordered) {
    if (!found.length || item.start >= found.at(-1).end - 1e-7) found.push(item);
  }
  return found;
}

function attacks(frames, fps) {
  const novelty = new Float32Array(frames.length);
  const bandAt = new Uint8Array(frames.length);
  const direction = new Float32Array(3 * frames.length);
  const last = [0, 0, 0];
  let maxFlux = 0;
  for (let i = 0; i < frames.length; i++) {
    let flux = 0, leader = 0;
    for (let k = 0; k < bands.length; k++) {
      const value = Number(frames[i]?.[bands[k]]);
      const level = Number.isFinite(value) ? Math.max(0, value) : 0;
      const rise = Math.max(0, level - last[k]);
      direction[3 * i + k] = rise;
      last[k] = level;
      flux += rise * rise;
      if (rise > direction[3 * i + leader]) leader = k;
    }
    flux = Math.sqrt(flux);
    novelty[i] = flux;
    bandAt[i] = leader;
    if (flux > 0) for (let k = 0; k < 3; k++) direction[i * 3 + k] /= flux;
    maxFlux = Math.max(maxFlux, flux);
  }
  const hits = [];
  const lookback = Math.max(4, Math.round(.65 * fps));
  const refractory = Math.max(2, Math.round(.09 * fps));
  let sum = 0;
  for (let i = 0; i < frames.length; i++) {
    const baseline = i ? sum / Math.min(i, lookback) : 0;
    const peak = novelty[i];
    if (peak >= Math.max(.0015, .035 * maxFlux, 1.6 * baseline)
        && peak > (novelty[i - 1] || 0) && peak >= (novelty[i + 1] || 0)) {
      const hit = { frame: i, band: bandAt[i], direction: direction.subarray(3 * i, 3 * i + 3) };
      const old = hits.at(-1);
      if (!old || i - old.frame >= refractory) hits.push(hit);
      else if (peak > novelty[old.frame]) hits[hits.length - 1] = hit;
    }
    sum += peak;
    if (i >= lookback) sum -= novelty[i - lookback];
  }
  return hits;
}

function motifSimilarity(reference, probe) {
  const scale = probe.span / reference.span;
  if (scale < .85 || scale > 1.18) return 0;
  let mismatch = 0;
  for (let k = 0; k < 3; k++) {
    const expected = reference.gaps[k] * scale;
    const error = Math.abs(expected - probe.gaps[k]);
    const tolerance = Math.max(2, expected * .16);
    if (error > tolerance) return 0;
    mismatch += error / tolerance;
  }
  let shape = 0;
  for (let k = 0; k < 4; k++) {
    const a = reference.hits[k].direction, b = probe.hits[k].direction;
    const cosine = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    if (cosine < .80) return 0;
    shape += Math.min(1, cosine);
  }
  // A similarity measure on captured envelopes, independent of the source gain.
  return .5 * (shape / 4) + .5 * (1 - mismatch / 3);
}

export function audioCueGroups(analysis) {
  if (!Array.isArray(analysis?.frames) || !Number.isFinite(analysis.fps) || analysis.fps <= 0) return [];
  const { frames, fps } = analysis;
  const hits = attacks(frames, fps);
  const candidates = new Map();
  for (let i = 0; i + 3 < hits.length; i++) {
    const four = hits.slice(i, i + 4);
    const gaps = four.slice(1).map((hit, j) => hit.frame - four[j].frame);
    if (gaps.some(gap => gap / fps > 1.5)) continue;
    const span = four[3].frame - four[0].frame;
    const key = four.map(hit => hit.band).join('');
    const probe = { hits: four, span, gaps };
    const bucket = candidates.get(key) || [];
    let best, quality = 0;
    for (const group of bucket) {
      const score = motifSimilarity(group.model, probe);
      if (score > quality) { best = group; quality = score; }
    }
    if (!best) {
      best = { model: probe, matches: [] };
      bucket.push(best);
      candidates.set(key, bucket);
      quality = 1;
    }
    const start = four[0].frame / fps, end = (four[3].frame + 1) / fps;
    best.matches.push({ start, end, confidence: quality });
  }
  const groups = [];
  for (const [key, bucket] of candidates) for (const { model, matches } of bucket) {
    const occurrences = nonOverlapping(matches);
    if (occurrences.length < 2) continue;
    groups.push({
      id: `audio-${key}-${model.hits[0].frame}`,
      label: `${model.hits.map(h => bandLabels[h.band]).join(' · ')} (${(model.span / fps).toFixed(2)} s)`,
      kind: 'audio', occurrences
    });
  }
  return groups.sort((a, b) => a.occurrences[0].start - b.occurrences[0].start || a.id.localeCompare(b.id));
}

function cleanText(raw) {
  return String(raw || '').replace(/<[^>]*>/g, '').replace(/&(?:amp|lt|gt|quot|apos|nbsp);/g,
    x => ({ '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&apos;': "'", '&nbsp;': ' ' })[x])
    .replace(/\[(?:music|instrumental|applause|laughter|singing|chorus|verse(?:\s+\d+)?)\]/gi, ' ')
    .replace(/[♪♫]/g, ' ').replace(/\s+/gu, ' ').trim();
}
function terms(text) {
  return (cleanText(text).normalize('NFKC').toLowerCase().replace(/[’ʼ]/g, "'")
    .match(/[\p{L}\p{N}]+(?:'[\p{L}\p{N}]+)*/gu) || []);
}
function timeCode(value) {
  if (typeof value === 'number') return value;
  if (typeof value !== 'string') return NaN;
  const s = value.trim().replace(',', '.');
  if (/^\d+(?:\.\d+)?$/.test(s)) return Number(s);
  const m = /^(?:(\d+):)?(\d{1,2}):([0-5]\d(?:\.\d+)?)$/.exec(s);
  return m && Number(m[2]) < 60 ? (Number(m[1]) || 0) * 3600 + Number(m[2]) * 60 + Number(m[3]) : NaN;
}
function timed(value, text) {
  const start = timeCode(value?.start), end = timeCode(value?.end);
  const label = cleanText(text);
  return Number.isFinite(start) && Number.isFinite(end) && start >= 0 && end > start && label
    ? { start, end, label } : null;
}

function readJson(text) {
  const root = JSON.parse(text);
  const rows = Array.isArray(root) ? root : root?.segments;
  const segments = [], words = [];
  const addWord = w => { const item = timed(w, w?.word ?? w?.text); if (item) words.push(item); };
  if (Array.isArray(rows)) for (const row of rows) {
    if (row?.word !== undefined && row?.text === undefined) { addWord(row); continue; }
    const item = timed(row, row?.text);
    if (item) segments.push(item);
    if (Array.isArray(row?.words)) row.words.forEach(addWord);
  }
  if (!words.length && Array.isArray(root?.words)) root.words.forEach(addWord);
  if (!Array.isArray(rows) && !Array.isArray(root?.words)) {
    throw new Error('Expected timed segments or words in the transcript JSON.');
  }
  return { segments, words };
}

function readCaptions(text) {
  const segments = [];
  for (const block of text.split(/\n[ \t]*\n/)) {
    const lines = block.split('\n');
    if (/^(?:NOTE(?:\s|$)|STYLE\s*$|REGION\s*$)/.test(lines[0])) continue;
    const index = lines.findIndex(line => line.includes('-->'));
    if (index < 0) continue;
    const times = /^\s*(\S+)\s*-->\s*(\S+)/.exec(lines[index]);
    if (!times) continue;
    const item = timed({ start: times[1], end: times[2] }, lines.slice(index + 1).join(' '));
    if (item) segments.push(item);
  }
  return { segments, words: [] };
}

export function lyricCueGroups(text, filename = '') {
  const content = String(text || '').replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').trim();
  if (!content) return [];
  const ext = filename.toLowerCase().split('.').pop();
  const { segments, words } = ext === 'json' || /^[{[]/.test(content) ? readJson(content) : readCaptions(content);
  const byPhrase = new Map();
  function add(label, start, end) {
    const tokens = terms(label), key = tokens.join(' ');
    if (!key) return;
    if (!byPhrase.has(key)) byPhrase.set(key, { label, tokens: tokens.length, matches: [] });
    byPhrase.get(key).matches.push({ start, end });
  }
  segments.forEach(s => add(s.label, s.start, s.end));
  // Package JSON and captions supply cue times. A match inside a coarse cue
  // gets that supplied interval; exact word times are retained when available.
  const timedWords = words.length ? words.filter(w => terms(w.label).length).sort((a, b) => a.start - b.start)
    : segments.slice().sort((a, b) => a.start - b.start).flatMap((s, block) => terms(s.label)
      .map(label => ({ label, start: s.start, end: s.end, block })));
  for (let i = 0; i < timedWords.length; i++) {
    let label = timedWords[i].label;
    for (let j = i + 1; j < Math.min(timedWords.length, i + 8); j++) {
      const current = timedWords[j], previous = timedWords[j - 1];
      const sameCaption = current.block !== undefined && current.block === previous.block;
      if (current.start - previous.end > .90 || (!sameCaption && current.end <= previous.end)) break;
      label += ' ' + timedWords[j].label;
      add(label, timedWords[i].start, timedWords[j].end);
    }
  }
  const repeated = [...byPhrase].map(([key, p]) => ({
    id: 'lyric-' + key, label: p.label, kind: 'lyric', size: p.tokens, occurrences: nonOverlapping(p.matches)
  })).filter(p => p.occurrences.length > 1).sort((a, b) => b.size - a.size || a.id.localeCompare(b.id));
  const kept = [];
  for (const phrase of repeated) {
    // Suppress substrings which describe exactly the same timed recurrences as a longer phrase.
    const covered = kept.some(longer => longer.size > phrase.size && longer.occurrences.length === phrase.occurrences.length &&
      (' ' + longer.id.slice(6) + ' ').includes(' ' + phrase.id.slice(6) + ' ') &&
      phrase.occurrences.every((p, i) => longer.occurrences[i].start <= p.start && longer.occurrences[i].end >= p.end));
    if (!covered) kept.push(phrase);
  }
  return kept.sort((a, b) => a.occurrences[0].start - b.occurrences[0].start || b.size - a.size)
    .map(({ size, ...publicGroup }) => publicGroup);
}
