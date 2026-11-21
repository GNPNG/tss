// Independently decode the published PNGs through FFmpeg and composite every pixel in linear RGB.
import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import assert from 'node:assert/strict';

const dir = process.argv[2];
if (!dir) throw Error('Usage: node verify-plates.mjs <plate bundle>');
const manifest = JSON.parse(readFileSync(join(dir, 'plates.json'), 'utf8'));
assert.equal(manifest.profile, 'DIGITAL_4K');
assert.deepEqual(manifest.size, [3840, 2160]);
const [width, height] = manifest.size;
const byteLength = width * height * 8;

function run(program, args, maxBuffer) {
  const result = spawnSync(program, args, {maxBuffer});
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(result.stderr.toString());
  return result.stdout;
}

const counts = {};
function readPlate(name, retain) {
  const path = join(dir, name);
  const [stream] = JSON.parse(run('ffprobe', ['-v', 'error','-show_streams','-of','json',path], 1024*1024)).streams;
  assert.deepEqual([stream.width, stream.height, stream.pix_fmt, stream.color_range, stream.color_space,
    stream.color_transfer, stream.color_primaries], [width, height, 'rgba64be', 'pc', 'gbr', 'bt709', 'bt709'], name);
  const raw = run('ffmpeg', ['-nostdin','-loglevel','error','-threads','1','-i',path,
    '-frames:v','1','-pix_fmt','rgba64le','-f','rawvideo','pipe:1'], byteLength + 65536);
  assert.equal(raw.byteLength, byteLength);
  const pixels = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);
  const seen = Array.from({length:4}, () => new Uint8Array(65536));
  for (let i = 0; i < raw.length; i += 8) {
    for (let c = 0; c < 4; c++) seen[c][pixels.getUint16(i+c*2,true)] = 1;
  }
  counts[name] = seen.map(bits => bits.reduce((sum,n) => sum+n,0));
  if (!['FG.png','MID.png'].includes(name)) {
    assert.equal(counts[name][3], 1, name);
    assert.equal(seen[3][65535], 1, name);
  }
  return retain ? pixels : null;
}

readPlate('rear-solo.png', false);
readPlate('front-solo.png', false);
const bg = readPlate('BG.png', true);
const mid = readPlate('MID.png', true);
const fg = readPlate('FG.png', true);
const projection = readPlate('projection.png', true);
assert.ok(counts['projection.png'].slice(0,3).every(count => count > 256), '8-bit projection precision');
const linear = Float64Array.from({length:65536}, (_, i) => {
  const v = i/65535;
  // BT.709's rounded branch constants leave a narrow interval at the junction.
  return v < 0.081 ? v/4.5 : Math.max(0.018, ((v+0.099)/1.099)**(1/0.45));
});
let maximum = 0, squared = 0;
for (let i = 0; i < byteLength; i += 8) {
  const midAlpha = mid.getUint16(i+6,true)/65535;
  const frontAlpha = fg.getUint16(i+6,true)/65535;
  for (let j = i; j < i+6; j += 2) {
    const back = linear[bg.getUint16(j,true)];
    const middle = back*(1-midAlpha) + linear[mid.getUint16(j,true)]*midAlpha;
    const composed = middle*(1-frontAlpha) + linear[fg.getUint16(j,true)]*frontAlpha;
    const difference = composed - linear[projection.getUint16(j,true)];
    maximum = Math.max(maximum, Math.abs(difference));
    squared += difference*difference;
  }
}
assert.ok(maximum < 0.0001, 'Linear-light reassembly differs from projection: '+maximum);
console.log(JSON.stringify({wheel:manifest.wheel, gpu:manifest.gpu, size:manifest.size,
  sourceFilmSeconds:[manifest.rearSeconds,manifest.frontSeconds], distinctCodesPerRGBAChannel:counts,
  maxAbsoluteLinearDifference:maximum, rmsLinearDifference:Math.sqrt(squared/(width*height*3))}, null, 2));
