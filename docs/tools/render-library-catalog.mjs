import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const entries = JSON.parse(readFileSync(new URL('library-catalog.json', root), 'utf8'));
const columns = ['group', 'name', 'model', 'stack', 'application', 'platform', 'scope', 'decision', 'evidence', 'sources'];
const names = new Set();
const groups = new Map();
for (const e of entries) {
  for (const key of columns.slice(0, -1)) {
    if (typeof e[key] !== 'string' || !e[key].trim()) throw new Error(`Missing ${key} in ${e.name}`);
  }
  if (!Array.isArray(e.sources) || !e.sources.length || names.has(e.name)) throw new Error(`Invalid row: ${e.name}`);
  names.add(e.name);
  if (!groups.has(e.group)) groups.set(e.group, []);
  groups.get(e.group).push(e);
}

const lines = [
  '# TSS library and demo catalog', '',
  `${entries.length} entries cover the libraries, demos, apparatus references and integration studies found in project research. Use the [sortable CSV](library-catalog.csv) to compare platform, source size, port decision and test evidence across groups. The [source data](library-catalog.json) drives both views.`, '',
  '## Material second pass', '',
  '| Material | First comparison | What determines whether to keep it |',
  '|---|---|---|',
  '| Oil and water | Two Basilisk gaps; 20 and 1,000 cSt PDMS in water | Preserve slide identity at crossings. Compare same-gap necking, timing and response to swirl against controlled footage. |',
  '| Wax | Taichi-LBM3D phase-change model versus MeltGL’s heat-dependent flow and moving outline | Compare melt-front travel, transmitted light and frame-time cost. Separate measured enthalpy from an artistic softness setting. |',
  '| Slime | Basilisk log-conformation, then a reduced stress-memory response | Compare stretch, overshoot and recoil under the same release. Keep material relaxation separate from gesture damping. |',
  '| Glass and hard sugar candy | Cycles or LuxCore refracted illumination from real geometry; Filament or the TSS live shader as display alternatives | Lock lamp, tint, thickness and exposure. Compare receiver caustics, crossed boundaries, and noise/flicker when moving. |',
  '| Hydrophobic sand | A granular bulk-flow candidate and a separate trapped-air optical surface | Compare aggregate buckling and breakage against the water-repellent sample. Check the submerged luster independently. |', '',
  'The [second-pass issue](https://gitea.gnomatix.com/gnpng/org/issues/13) tracks these comparisons. A narrow source count refers to the named file, kernel, adapter or fixture in that row. It does not estimate the size of a full library or the work to support a production port. Test evidence applies to the stated workload and machine. An upstream capability remains a candidate until tested in the relevant TSS stage.', '',
  '## Index', ''
];
for (const [group, records] of groups) {
  const id = group.toLowerCase().replace(/[^a-z0-9 -]/g,'').replace(/ +/g,'-');
  lines.push(`- [${group}](#${id}) (${records.length})`);
}
const cell = s => s.replaceAll('|', '&#124;').replace(/\s*\n\s*/g, ' ');
for (const [group, records] of groups) {
  lines.push('', `## ${group}`, '', '| Resource and sources | Model, language and APIs | TSS use and integration decision | Platform constraints and test evidence | Size and complexity |',
    '|---|---|---|---|---|');
  for (const e of records) lines.push(`| **${cell(e.name)}**<br>${e.sources.map(cell).join(' · ')} | ${cell(e.model)}<br><br>${cell(e.stack)} | ${cell(e.application)}<br><br>${cell(e.decision)} | ${cell(e.platform)}<br><br>${cell(e.evidence)} | ${cell(e.scope)} |`);
}
writeFileSync(new URL('library-catalog.md',root), lines.join('\n')+'\n');
const quote = s => '"'+s.replaceAll('"','""')+'"';
const csvRows = [columns, ...entries.map(e => columns.map(k => k==='sources' ? e.sources.join(' ; ') : e[k]))];
writeFileSync(new URL('library-catalog.csv',root), csvRows.map(row => row.map(quote).join(',')).join('\r\n')+'\r\n');
