const label = {energy_rise:'Rising energy', recurrence:'Each occurrence'};
const choiceName = id => label[id] || id[0].toUpperCase() + id.slice(1);

export function installVisualAdvisor({invoke, selection, apply, source, selectedLyrics}) {
  const el = id => document.getElementById('advice-' + id);
  const status = el('status'), review = el('review');
  const direction = el('direction');
  let suggested = null, waiting = false;
  el('panel').hidden = false;

  const axes = ['visual','signal','fit'];
  const catalog = fetch(new URL('./assets/visual-advice.json', import.meta.url))
    .then(r => { if (!r.ok) throw new Error('Cannot load advice catalog.'); return r.json(); });
  // Show failures at the initiating control, even if initialization failed before the first click.
  catalog.catch(() => {});
  function sameSelection(old) {
    const now = selection();
    return old && old.song === source() && now && now.start === old.start && now.end === old.end;
  }
  function applyState() {
    el('apply').disabled = !suggested || el('visual').value !== 'oil' || !el('cue').value.trim();
  }
  el('visual').addEventListener('change',applyState);
  el('cue').addEventListener('input',applyState);
  el('suggest').addEventListener('click', async () => {
    if (waiting) return;
    const range = selection(), song = source(), text = direction.value.trim();
    if (!range || !song || !(range.start >= 0 && range.end > range.start)) {
      status.textContent = 'Select a timeline region or Select all first.'; return;
    }
    let lyrics;
    try { lyrics = el('lyrics').checked ? selectedLyrics(range) : ''; }
    catch (e) { status.textContent = e.message; return; }
    if (!text && !lyrics) { status.textContent = 'Enter a visual direction or include reviewed lyrics from this region.'; return; }
    const request = [text, lyrics && 'Lyric excerpt: ' + lyrics].filter(Boolean).join('\n');
    if ([...request].length > 1600) { status.textContent = 'Shorten the direction or select a smaller lyric range (1600 characters maximum).'; return; }
    const snapshot = {song,start:range.start,end:range.end};
    suggested = null;
    review.hidden = true;
    waiting = true; el('suggest').disabled = true;
    status.textContent = 'Consulting the local advisor…';
    try {
      const catalogValue = await catalog;
      const result = await invoke('suggest_visual_layer', {direction:request, port:el('port').valueAsNumber});
      if (!sameSelection(snapshot)) throw new Error('The selection changed. Request advice for the new region.');
      for (const axis of axes) {
        if (!Object.hasOwn(catalogValue[axis],result[axis])) throw new Error('Unrecognized advisor choice.');
        const select = el(axis);
        select.replaceChildren(...Object.keys(catalogValue[axis]).map(id => new Option(choiceName(id),id)));
        select.value = result[axis];
      }
      el('cue').value = text || lyrics;
      el('range').textContent = range.start.toFixed(2) + '–' + range.end.toFixed(2) + ' s';
      suggested = snapshot;
      review.hidden = false;
      applyState();
      status.textContent = 'Review the proposed layer, trigger and fit.';
    } catch (e) { status.textContent = String(e.message || e); }
    finally { waiting = false; el('suggest').disabled = false; }
  });
  el('apply').addEventListener('click', async () => {
    if (!sameSelection(suggested)) { status.textContent = 'Request advice for the current selection first.'; return; }
    const cue = el('cue').value.trim();
    if (!cue || el('visual').value !== 'oil') return;
    try {
      await apply({start:suggested.start,end:suggested.end,visual:'oil',signal:el('signal').value,
        fit:el('fit').value,cue,label:cue});
      suggested = null; applyState(); status.textContent = 'Added the reviewed oil layer.';
    } catch (e) { status.textContent = String(e.message || e); }
  });
}
