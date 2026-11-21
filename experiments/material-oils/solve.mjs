// Run the existing Basilisk SI solver on one independently initialized full slide.
import {existsSync, mkdirSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {spawnSync} from 'node:child_process';

const [solver, mask, directory, id, count = '96', level = '7', slide = 'cyan'] = process.argv.slice(2);
const registry = JSON.parse(readFileSync(new URL('../../live-wheel/materials/fluids-and-glass.json', import.meta.url)));
const oil = registry.oils.find(o => o.id === id);
const integerIn = (value, min, max) => /^\d+$/.test(value) && Number(value) >= min && Number(value) <= max;
if (!['cyan','violet'].includes(slide) || !oil || !solver || !mask || !directory ||
    existsSync(directory) || !integerIn(count, 1, 240) || !integerIn(level, 7, 9)) {
  throw Error('Usage: node solve.mjs SOLVER MASK.pgm NEW_DIR pdms-water-20cst|pdms-water-1000cst [FRAMES] [LEVEL] [cyan|violet]');
}
mkdirSync(directory);
const physical = [
  0.050, 0.050, 0.0025,
  oil.density_kg_m3, oil.dynamic_viscosity_pa_s,
  oil.water_density_kg_m3, oil.water_dynamic_viscosity_pa_s,
  oil.oil_water_interfacial_tension_n_m,
];
const args = [resolve(mask), resolve(directory), slide, count, level, '--si', ...physical.map(String), '--world'];
console.error(JSON.stringify({material:id, args}));
const run = spawnSync(resolve(solver), args, {stdio:'inherit', env:{...process.env, OMP_NUM_THREADS:'1'}});
if (run.error) throw run.error;
process.exitCode = run.status ?? 1;
