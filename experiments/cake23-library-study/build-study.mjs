// Build a study-only host around the original Cake23 graphics code.
// Upstream source and license remain in vendor/Cake23; this does not implement physics.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const vendor = join(root, 'vendor', 'Cake23');
const output = join(root, 'output');
const names = ['lavalamp', 'reaction-diffusion-spot-worms'];
const hash = (text) => createHash('sha256').update(text).digest('hex');

// Extract each complete shader script and graphics host script without rewriting either.
function shaders(text) {
  return [...text.matchAll(/<script\b[^>]*type="x-shader\/[^>]*>[\s\S]*?<\/script>/g)].map((m) => m[0]);
}
function graphicsHost(text) {
  const marker = '<script type="text/javascript">';
  const first = text.indexOf(marker);
  const second = text.indexOf(marker, first + marker.length);
  if (first < 0 || second < 0) throw new Error('Expected upstream JavaScript host scripts');
  const end = text.indexOf('</script>', second);
  if (end < 0) throw new Error('Missing graphics host script terminator');
  return text.slice(second, end + '</script>'.length);
}
function makeStudy(source, name) {
  let html = source;
  const sourceTags = [...html.matchAll(/^\s*<script src="@\(Model\.Host\.URL\)\/[^\"]+"><\/script>\r?\n/gm)];
  if (sourceTags.length !== 3) throw new Error(`${name}: expected exactly three Razor host scripts`);
  html = html.replace(/^\s*<script src="@\(Model\.Host\.URL\)\/[^\"]+"><\/script>\r?\n/gm, '\n');

  const start = html.indexOf('\t\t$(function () {');
  const endToken = '\n\t\t});';
  const end = html.indexOf(endToken, start);
  if (start < 0 || end < 0) throw new Error(`${name}: could not locate upstream SignalR bootstrap`);
  const hookup = html.slice(start, end + endToken.length);
  if (!hookup.includes('kinect2Hub') || !hookup.includes('midiHub')) throw new Error(`${name}: unexpected bootstrap`);
  html = html.slice(0, start) + '\t\t// Study host has no Kinect/MIDI SignalR integration.\n' + html.slice(end + endToken.length);
  if (html.includes('@(Model.Host.URL)') || html.includes('$.connection')) throw new Error(`${name}: unresolved server hookup`);

  const before = shaders(source);
  const after = shaders(html);
  if (before.length === 0 || before.length !== after.length || before.some((s, i) => s !== after[i])) {
    throw new Error(`${name}: upstream shader scripts changed`);
  }
  if (graphicsHost(source) !== graphicsHost(html)) throw new Error(`${name}: upstream graphics host changed`);
  const note = '<div style="position:fixed;z-index:2147483647;bottom:0;left:0;background:#111;color:white;padding:6px;font:12px sans-serif">Upstream visual study; Kinect/MIDI SignalR unavailable; not a confined-slide simulation.</div>';
  if (!html.includes('</body>')) throw new Error(`${name}: missing body close`);
  return { html: html.replace('</body>', `${note}\n</body>`), shaderCount: before.length, shaderHashes: before.map(hash), hostHash: hash(graphicsHost(source)) };
}

await mkdir(output, { recursive: true });
const report = [];
for (const name of names) {
  const source = await readFile(join(vendor, 'Cake23', 'WebTemplates', `${name}.cshtml`), 'utf8');
  const result = makeStudy(source, name);
  const outputPath = join(output, `${name}.html`);
  await writeFile(outputPath, result.html, 'utf8');
  report.push({ name, sourceSha256: hash(source), htmlSha256: hash(result.html), shaderCount: result.shaderCount, shaderHashes: result.shaderHashes, graphicsHostSha256: result.hostHash });
}
await writeFile(join(output, 'provenance.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report.map(({ name, sourceSha256, htmlSha256, shaderCount, graphicsHostSha256 }) => ({ name, sourceSha256, htmlSha256, shaderCount, graphicsHostSha256 })), null, 2));
