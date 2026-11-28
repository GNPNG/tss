import { equipmentFor } from './equipment.js';
import { createLegendLibrary, MAX_PACK_BYTES, parseLegendPack } from './legend-packs.js';

const reviewLabels = {
  original: 'Original rig',
  research: 'Research draft',
  'artist-contributed': 'Artist contribution',
  'artist-reviewed': 'Artist reviewed'
};

function element(tag, text, className) {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

function button(text, onClick) {
  const control = element('button', text, 'button quiet');
  control.type = 'button';
  control.addEventListener('click', onClick);
  return control;
}

function buildRig(entry, visible) {
  const { rig } = entry;
  const routes = new Map();
  const beams = new Set(visible.map(layer => layer.projector ?? 'a'));
  if (visible.length > 1 && beams.size === 1) {
    const beam = beams.has('a') ? 'b' : 'a';
    routes.set(beam === 'b' ? visible[0].id : visible.at(-1).id, beam);
  }
  return { routes, focus: rig.focusA, fields: {
    equipmentA: equipmentFor(rig.a), lampKelvin: rig.temperatureA, projectorAIntensity: 1,
    projectorB: { equipment: equipmentFor(rig.b), enabled: true, lampKelvin: rig.temperatureB,
      focus: rig.focusB, intensity: rig.levelB, offsetX: rig.offsetX, offsetY: 0 }
  } };
}

export function installLegends({ layers, preview, apply, availablePacks = async () => [] }) {
  const $ = id => document.getElementById(id);
  const library = createLegendLibrary();
  const carousel = $('legend-carousel'), cards = $('legend-cards'), file = $('legend-file');
  const message = $('legend-pack-status');
  let entries = [], position = 0, audition = null, replacement = null, catalog = [], catalogRequest = 0;

  function showAudition() {
    $('legend-status').textContent = audition ? 'Auditioning ' + audition.artist + '. Apply to save the rig.' : '';
    $('legend-stop').hidden = !audition;
    $('legend-preview').hidden = !audition;
    $('legend-preview-artist').textContent = audition ? 'Auditioning ' + audition.artist : '';
    const control = cards.querySelector('[data-audition]');
    if (control) control.setAttribute('aria-pressed', String(audition?.id === control.dataset.audition));
  }

  function stop() {
    if (!audition) return;
    audition = null;
    showAudition();
    preview();
  }

  function revealPreview() {
    const stage = $('preview');
    const bounds = stage.getBoundingClientRect();
    if (bounds.bottom <= 0 || bounds.top >= innerHeight) {
      stage.scrollIntoView({block: 'center'});
      if (audition) $('legend-back').focus({preventScroll: true});
    }
  }

  function showCard() {
    cards.replaceChildren();
    const current = entries[position];
    $('legend-empty').hidden = !!current;
    carousel.hidden = !current;
    if (!current) return;
    const { pack, entry, key } = current;
    $('legend-position').textContent = (position + 1) + ' of ' + entries.length + ' · ' + entry.artist;
    $('legend-prev').disabled = $('legend-next').disabled = entries.length < 2;
    const card = element('div', undefined, 'legend-card');
    card.setAttribute('role','group');
    card.setAttribute('aria-roledescription','slide');
    card.setAttribute('aria-labelledby','legend-title');
    const name = element('h3', entry.artist);
    name.id = 'legend-title';
    const diagram = element('div', undefined, 'rig-diagram');
    diagram.setAttribute('aria-hidden','true');
    for (const profile of [entry.rig.a, entry.rig.b]) diagram.append(element('i', undefined, 'source-icon ' + profile));
    const sources = element('div', undefined, 'rig-actions');
    for (const [label, href] of entry.sources) {
      const link = element('a', label + ' ↗');
      link.href = href; link.target = '_blank'; link.rel = 'noopener noreferrer';
      sources.append(link);
    }
    const setup = element('p');
    setup.append(element('strong', 'Simulation setup. '), entry.setup);
    const credit = element('p', 'Credits · ' + pack.credits.join(', '), 'legend-credit');
    const review = element('p', 'Review stated by pack · ' + reviewLabels[pack.review.status] + '. ' + pack.review.note, 'legend-credit');
    const controls = element('div', undefined, 'rig-actions');
    const materialize = () => ({ ...buildRig(entry, layers().filter(layer => layer.visible)),
      id: key, packId: pack.id, artist: entry.artist });
    const listen = button('Audition', () => {
      audition = materialize();
      showAudition();
      revealPreview();
      preview();
    });
    listen.dataset.audition = key;
    const use = button('Apply rig', () => {
      const selected = materialize();
      audition = null;
      showAudition();
      revealPreview();
      apply(selected, entry.artist);
    });
    use.dataset.apply = key;
    controls.append(listen, use);
    card.append(diagram, name, element('small', entry.venue), element('p', entry.record),
      sources, setup, controls, element('p', pack.title + ' · v' + pack.version, 'legend-pack-title'), credit, review);
    cards.append(card);
    showAudition();
  }

  function showPacks(packs) {
    const list = $('legend-packs');
    list.replaceChildren();
    $('legend-installed').hidden = !packs.length;
    for (const pack of packs) {
      const row = element('div', undefined, 'rig-actions legend-pack-row');
      const toggle = element('input');
      toggle.type = 'checkbox';
      toggle.checked = library.isEnabled(pack.id);
      toggle.dataset.enable = pack.id;
      toggle.setAttribute('aria-label','Enable ' + pack.title);
      toggle.addEventListener('change', () => {
        try {
          library.setEnabled(pack.id, toggle.checked);
          if (!toggle.checked && audition?.packId === pack.id) stop();
          refresh(toggle.checked ? pack.id : undefined);
          list.querySelector('[data-enable="' + CSS.escape(pack.id) + '"]').focus();
        } catch (error) { toggle.checked = library.isEnabled(pack.id); message.textContent = error.message; }
      });
      const label = element('label', undefined, 'rig-actions');
      label.append(toggle, 'Enabled');
      const remove = button('Remove pack', () => {
        try {
          library.remove(pack.id);
          if (audition?.packId === pack.id) stop();
          clearReplacement();
          refresh();
          message.textContent = 'Removed ' + pack.title + '.';
          (list.querySelector('input') ?? $('legend-install')).focus();
        } catch (error) { message.textContent = error.message; }
      });
      remove.dataset.remove = pack.id;
      remove.setAttribute('aria-label','Remove pack ' + pack.title);
      row.append(element('span', pack.title + ' · v' + pack.version), label, remove);
      list.append(row);
    }
  }

  function showAvailable(packs) {
    const list = $('legend-available-list');
    list.replaceChildren();
    $('legend-available').hidden = !catalog.length;
    $('legend-empty').textContent = packs.length ? 'Enable a pack to browse its rigs.'
      : catalog.length ? 'Choose an available pack or import one to audition a rig with your current tracks.'
      : 'Import a Legends pack (.tsslegends.json) to audition a rig with your current tracks.';
    for (const { pack, text } of catalog) {
      const installed = packs.find(item => item.id === pack.id);
      const identical = !!installed && JSON.stringify(installed) === JSON.stringify(pack);
      const row = element('div', undefined, 'rig-actions legend-pack-row');
      const action = button(identical ? 'Installed' : installed ? 'Replace pack…' : 'Install pack', () => {
        try {
          requestInstall(text, pack);
          if (!replacement) (carousel.hidden ? $('legend-install') : carousel).focus();
        } catch (error) { message.textContent = error.message; }
      });
      action.disabled = identical;
      action.dataset.available = pack.id;
      action.setAttribute('aria-label', action.textContent + ' · ' + pack.title);
      row.append(element('span', pack.title + ' · v' + pack.version + ' · ' + reviewLabels[pack.review.status]), action);
      list.append(row);
    }
  }

  async function readAvailable() {
    const request = ++catalogRequest;
    try {
      const files = await availablePacks();
      if (request !== catalogRequest) return;
      const accepted = [], errors = [];
      for (const { fileName, text } of files) {
        try { accepted.push({ pack: parseLegendPack(text), text }); }
        catch (error) { errors.push(fileName + ' — ' + error.message); }
      }
      catalog = accepted;
      showAvailable(library.packs);
      $('legend-catalog-status').textContent = errors.join(' · ');
    } catch (error) {
      if (request !== catalogRequest) return;
      catalog = [];
      showAvailable(library.packs);
      $('legend-catalog-status').textContent = 'Could not read available packs. ' + (error.message ?? error);
    }
  }

  function refresh(selectPack) {
    const previous = entries[position]?.key;
    const packs = library.packs;
    entries = packs.filter(pack => library.isEnabled(pack.id))
      .flatMap(pack => pack.rigs.map(entry => ({ pack, entry, key: pack.id + '/' + entry.id })));
    const target = entries.findIndex(item => selectPack ? item.pack.id === selectPack : item.key === previous);
    position = target >= 0 ? target : Math.max(0, Math.min(position, entries.length - 1));
    showPacks(packs);
    showAvailable(packs);
    showCard();
  }

  function navigate(index) {
    if (!entries.length) return;
    const cardHadFocus = cards.contains(document.activeElement);
    position = (index + entries.length) % entries.length;
    showCard();
    if (cardHadFocus) carousel.focus();
  }

  function clearReplacement() {
    replacement = null;
    $('legend-replacement').hidden = true;
  }

  function install(text, replace = false) {
    const pack = library.install(text, { replace });
    if (audition?.packId === pack.id) stop();
    clearReplacement();
    refresh(pack.id);
    message.textContent = 'Installed ' + pack.title + ' v' + pack.version + '.';
  }

  function requestInstall(text, pack) {
    clearReplacement();
    const existing = library.packs.find(item => item.id === pack.id);
    if (!existing) { install(text); return; }
    replacement = text;
    message.textContent = 'Replace ' + existing.title + ' v' + existing.version + ' with ' + pack.title + ' v' + pack.version + '?';
    $('legend-replacement').hidden = false;
    $('legend-keep').focus();
  }

  $('legends').addEventListener('toggle', () => { if ($('legends').open) readAvailable(); });
  $('legend-install').addEventListener('click', () => file.click());
  file.addEventListener('change', async () => {
    const selected = file.files[0];
    if (!selected) return;
    clearReplacement();
    file.value = '';
    $('legend-install').disabled = true;
    try {
      if (selected.size > MAX_PACK_BYTES) throw Error('Pack exceeds the file size limit.');
      const text = await selected.text();
      requestInstall(text, parseLegendPack(text));
    } catch (error) { message.textContent = error.message; }
    finally { $('legend-install').disabled = false; }
  });
  $('legend-keep').addEventListener('click', () => {
    clearReplacement();
    message.textContent = 'Kept the installed pack.';
    $('legend-install').focus();
  });
  $('legend-replace').addEventListener('click', () => {
    if (!replacement) return;
    try { install(replacement, true); $('legend-install').focus(); }
    catch (error) { message.textContent = error.message; }
  });
  $('legend-prev').addEventListener('click', () => navigate(position - 1));
  $('legend-next').addEventListener('click', () => navigate(position + 1));
  carousel.addEventListener('keydown', event => {
    if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const destinations = { ArrowLeft: position - 1, ArrowRight: position + 1, Home: 0, End: entries.length - 1 };
    if (!Object.hasOwn(destinations, event.key)) return;
    event.preventDefault(); event.stopPropagation();
    navigate(destinations[event.key]);
  });
  $('legend-stop').addEventListener('click', () => {
    stop();
    (cards.querySelector('[data-audition]') ?? $('legend-install')).focus({preventScroll:true});
  });
  function backToRig() {
    const target = entries.findIndex(item => item.key === audition?.id);
    if (target >= 0) navigate(target);
    $('legends').open = true;
    (cards.querySelector('[data-audition]') ?? $('legend-install')).focus();
  }
  $('legend-back').addEventListener('click', backToRig);
  $('legend-preview-stop').addEventListener('click', () => { backToRig(); stop(); });
  refresh();
  if (library.error) message.textContent = library.error;
  return { get audition() { return audition; }, stop };
}
