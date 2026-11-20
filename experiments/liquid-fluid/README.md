# Liquid-fluid experiment

This isolated research prototype is **not integrated into TSS and has not been approved as a match to the reference imagery**. Its output currently looks too much like flat translucent spots; it lacks the nested interfaces, marbling, optical structure, and motion of the supplied Mathmos image and reference C. It is not a photorealistic or physically validated oil/water simulation.

## Run the checks

Serve this directory over HTTP using Bun (no install or network access is needed once Bun is present):

```sh
cd experiments/liquid-fluid
bun -e 'Bun.serve({port:19876,hostname:"127.0.0.1",fetch(req){return new Response(Bun.file("."+new URL(req.url).pathname))}})'
```

Open `http://127.0.0.1:19876/layer-check.html`, `gesture-check.html`, and `float-check.html` in a browser with WebGL2 and float rendering and readback. Results appear below each canvas. A completed run should show `glError: 0`, `contextLost: false` where listed, all named checks true, with `changedBySeed` and `impacted` matching the arrays below. Failures, unsupported GPU features and compilation errors appear on the page. Stop the server with Ctrl-C. The server is for local use only.

- `layer-check.html`: color changes leave all fields unchanged; changing the first seed changes only its field (`[true,false,false,false]`); all four fields evolve, deterministic replay agrees, and disabling all four yields white.
- `gesture-check.html`: concurrent pencil and trackpad events on different layers affect `[true,false,true,false]`; pencil pressure does not change default swirl, trackpad compression responds to pressure, replay agrees, and simultaneous sources on the same layer each contribute. A short down/move/up within a step is retained.
- `float-check.html`: checks genuine `Float32Array` framebuffer readback, opaque alpha, value range and conversion against the 8-bit preview. These checks do not establish 16-bit file production.

On this Intel UHD 630 Mac, Chrome with ANGLE Metal passed the three pages. At 256² simulation per layer, the first layer-check initialization took 2332 ms and first 30 Hz simulation step 167 ms. Another 1280×720, 512²-per-layer, 60-frame capture reported 11.5 s setup and 203.6 ms first step; its warm-step timing excluded PNG capture and cannot establish live or 4K throughput. Test on target hardware and resolution before making performance decisions.

## Model and interface

Four independent `GpuFluid` objects each own concentration, velocity and pressure state. The compositor interprets the scalar alpha concentration as optical thickness and combines the four colored filters with an approximate RGB Beer–Lambert transmission in linear light; weak gradient-derived refraction is optional. The RGB coefficients are not spectrally calibrated. The layers share no phase-coupled velocity or pressure and interact optically only; they do not model immiscibility, coupled surface tension or validated fluid optics. This model represents an approximation of stacked transmissive media, not additive mixing of separate projectors. Historical context: [Michael Scroggins oral history](https://www.alternativeprojections.com/oral-histories/michael-scroggins/).

`LiquidRenderer(canvas, {simulationSize, seed})` accepts WebGL2 canvas dimensions independently of its square simulation grid. `render({time, featureAt, palette, layerSettings, gestureEvents, gestureRevision, gestureBindings})` advances at fixed 30 Hz in up to four steps; larger jumps require `await seek(time, {featureAt, ..., signal, onProgress})`. Seeking reinitializes and replays from zero asynchronously; it has no checkpoints and long seeks may take substantial time. For timeline equivalence, provide the same time-indexed `featureAt(t)`, simulation settings, recorded events and source bindings for preview and export; the renderer cannot reconstruct prior audio features supplied only as instantaneous arguments. Determinism was checked within one browser/GPU, not across devices or drivers.

Each `layerSettings` entry has `id`, `seed`, `color` (`#rrggbb` or encoded sRGB triplet), `strength`, `motion`, `sharpness`, `refraction`, and `enabled`. Color and strength operate at composition; changing a seed or simulation controls requires replay. `readLayer(i)` exposes float simulation readback for inspection. `readLinearFrame(width,height)` composites directly to an RGBA32F framebuffer and returns an opaque, bottom-up float array in linear sRGB/Rec.709 primaries. It does not encode a PNG, produce transparent BG/mid/FG plates, establish 16-bit file compliance, or provide Display P3 or CMYK output. The internal fluid texture precision is device dependent. A transmissive filter cannot in general be reproduced on arbitrary backdrops by ordinary straight-alpha compositing.

## Gesture experiment

Input records are compatible with the separate `experiments/gesture-input` study: `{time, sourceId, pointerId, pointerType, phase, x, y, pressure, layerIds}`. The prototype accepts arbitrarily many source identities and event records, constrained by hardware and memory; it does not claim simultaneous physical devices have been tested. A `gestureBindings` object maps source IDs to `swirl`, `compress`, `both`, or `ignore`. Default pen movement adds velocity and default mouse pressure applies a local negative gap delta to the solver; pen pressure does not control swirl. This is an experimental forcing, not validated plate compression. Pressure, tilt, Apple Pencil and trackpad hardware behavior need device-specific validation; tilt is recorded by capture but unused by this solver.

Events must be sorted by nonnegative timeline time. Equal-time events retain their input list order; active source forces are accumulated for each layer in the simulation step. Conflicting simultaneous forces have no separately validated physical priority or cancellation rule. When prior recorded events or bindings change, increment `gestureRevision` and replay with `seek`. Event coordinates are normalized to the preview element and transformed using the **current canvas aspect ratio and scale**. Replay on a differently shaped canvas will therefore differ: a production recorder must persist plate-space coordinates or the original viewport transform before claiming preview/export equivalence across aspect ratios.

## Upstream source and license

`gpu-fluid.ts` is derived from [ChromaGlass `src/lib/gpuFluid.ts`](https://github.com/jstevoh/chromaglass/blob/9e6c54fe274b775a9decbed282350b0adfb05b65/src/lib/gpuFluid.ts), commit `9e6c54fe274b775a9decbed282350b0adfb05b65`, the MIT-licensed v1.2.0 revision before its license change. Its sole source modification is the public `dyeTexture` accessor. `gpu-fluid.js` is the browser bundle of that source and contains the complete MIT notice. The unabridged license and copyright notice are in `chromaglass-MIT.txt`. No code from the later BSL revisions is included.
