// Prepare the offline Basilisk fields used for interactive rendering.
// Input is the lossless two-distance stream from slide-look-study/pack-sequence.sh.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

const [input, output] = process.argv.slice(2);
if (!input || !output) throw new Error('Usage: node make-field-cache.mjs performance.mkv slide-fields.rg8.gz');
const stream = JSON.parse(execFileSync('ffprobe', ['-v', 'error', '-select_streams', 'v:0',
  '-show_entries', 'stream=width,height,pix_fmt,avg_frame_rate,color_range', '-of', 'json', input])).streams[0];
if (stream.width !== 1024 || stream.height !== 576 || stream.pix_fmt !== 'gbrp16le' ||
    stream.color_range !== 'pc' || stream.avg_frame_rate !== '24/1')
  throw new Error('Expected the full-range 1024×576, 24 fps lossless RGB distance sequence');
const width = 512, height = 288, frames = 96, fps = 24;
const pixelCount = width * height * frames;
const rgb = execFileSync('ffmpeg', ['-v', 'error', '-nostdin', '-i', input, '-an', '-threads', '1',
  '-filter_threads', '1', '-vf', 'scale=512:288:flags=area:in_range=full:out_range=full',
  '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1'], { maxBuffer: pixelCount * 3 + 65536 });
if (rgb.length !== pixelCount * 3) throw new Error('Unexpected decoded frame count');
const cache = Buffer.alloc(24 + pixelCount * 2);
cache.write('TSSDF001', 0, 'ascii');
[width, height, frames, fps].forEach((v, i) => cache.writeUInt32LE(v, 8 + i * 4));
for (let i = 0; i < pixelCount; ++i) {
  if (rgb[i * 3 + 2] > 1) throw new Error('Unexpected nonzero reserved field channel');
  cache[24 + i * 2] = rgb[i * 3];
  cache[24 + i * 2 + 1] = rgb[i * 3 + 1];
}
writeFileSync(output, gzipSync(cache, { level: 6 }), { flag: 'wx' });

