import { mkdirSync, readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const catalog = JSON.parse(readFileSync(
  new URL('../../live-wheel/materials/fluids-and-glass.json', import.meta.url), 'utf8'));
const [solver, mask, output, slide, id, widthMM, speedMMs, gapMM, frames = '96', level = '9'] = process.argv.slice(2);
const oil = catalog.oils.find(entry => entry.id === id);
const [width, speed, gap] = [widthMM, speedMMs, gapMM].map(value => Number(value) / 1000);
const integerIn = (value, low, high) => /^\d+$/.test(value) && Number(value) >= low && Number(value) <= high;
if (!output || process.argv.length > 12 || !['violet', 'cyan'].includes(slide) || !oil ||
    ![width, speed, gap].every(value => Number.isFinite(value) && value > 0) || gap >= width ||
    !integerIn(frames, 1, 240) || !integerIn(level, 7, 9)) {
  console.error('Usage: node solve-material.mjs SOLVER MASK.pgm NEW_OUTDIR violet|cyan MATERIAL WIDTH_MM VELOCITY_SCALE_MM_S GAP_MM [FRAMES] [LEVEL]');
  console.error('Materials:', catalog.oils.map(entry => entry.id).join(', '));
  process.exit(2);
}

if (!statSync(solver).isFile() || !statSync(mask).isFile()) throw new Error('Expected solver and mask files');
mkdirSync(output);
const physical = [width, speed, gap, oil.density_kg_m3, oil.dynamic_viscosity_pa_s,
  oil.water_density_kg_m3, oil.water_dynamic_viscosity_pa_s, oil.oil_water_interfacial_tension_n_m];
console.error(`Slide ${slide}, material ${oil.id}, source ${catalog.sources.oil_water}`);
const run = spawnSync(resolve(solver), [resolve(mask), resolve(output), slide, frames, level,
  '--si', ...physical.map(String)], { stdio: 'inherit' });
if (run.error) throw run.error;
process.exitCode = run.status ?? 1;
