# Live Mathmos wheels on Windows

From the repository root, build and launch the desktop editor in PowerShell. Building requires the project's Rust 1.88 toolchain, the Windows Tauri prerequisites and `glslc` from the Vulkan SDK (`VULKAN_SDK`, `PATH`, or the compiler path in `GLSLC`). The executable uses the installed Vulkan graphics driver. CUDA is unnecessary. [Windows Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows)

```powershell
cargo run --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
```

The Windows host enumerates hardware adapters that can present to the editor and prefers a discrete GPU. To select one, set `$env:TSS_LIVE_ADAPTER = 'GTX 970'` before launching. The filter accepts a case-insensitive substring, including names of integrated and non-NVIDIA GPUs. A GTX 970 with a suitable Vulkan driver can be tried through this same build. The live readout identifies the adapter and reports its measured cadence.

## Performance controls

1. Select a visible liquid track and choose a preset under **Mathmos oil wheels**. **Live selected wheel** opens that track in the native preview. Windows starts with Violet / Blue.
2. Choose **Rear slide** or **Front slide** under **Mouse target**. Choose **Material tug** or **Rotate slide** under **Mouse action**, then hold the left button and drag on the native image. A tug pulls the selected liquid locally. **Material response** sets **Heavy**, **Medium**, or **Light** for that slide. Rotation turns the whole selected slide about the optical center. Releasing the button holds the pose. Each track keeps two independent slide poses.
3. Select **640 × 360** or **960 × 540** under **Live resolution**. These cap the optical render width. A smaller viewport renders at its own width. The last pass scales the optical image to the stage.
4. Move **Projector focus** between the rear and front planes. It overrides the default fitted focus across both slides and tracks. **Restore wheel focus** returns to the spatially varying focus used in the wheel study. **View study** opens the recorded reference. The live readout under the stage shows adapter, optical resolution, FPS and p95 frame interval.

The native viewport shows the two sealed slides on the selected track. Its fluid trajectory advances on a performance clock. The gesture deck can replay live inputs against the song.

## Export the selected wheel

Open a local song, choose the live wheel, then select **Export selected wheel in 4K**. Choose a new MP4 filename. The current wheel material and focus apply throughout. Enabled gestures scheduled at a recording position, audio motif or lyric phrase replay in song order at 30 fps. **Export starting pose** offers three choices.

- **Automatic** starts from neutral slides and the beginning of the fluid cache when the song contains scheduled events for this track. An unscheduled wheel starts from its current poses and visible film position.
- **Current pose and film position** copies the two live slides, then applies the scheduled events to that copy.
- **Neutral pose and film start** starts at cache time zero with both slide rotations and tugs cleared.

The native thread suspends presentation and uses the same GPU for one 3840 × 2160 optical frame at a time. The simulation source retains its 704 × 704 full-slide cache and 512 × 288 gate. Each export samples that geometry through the wheel optics at the output resolution. Enabled video feedback starts with an empty history and follows song time. FFmpeg receives the full-range sRGB RGBA8 output through a native pipe and converts to limited-range BT.709 H.264, 8-bit 4:2:0, with AAC audio. Completion and cancellation restore the original slide poses, focus, preview size and feedback history. **Export procedural preview** exports the procedural timeline composite.

## Computation and source data

The bundled [complete-slide cache](assets/slide-world.rg8.gz) holds 96 Basilisk states at 24 Hz in a 704 × 704 world, with one unsigned 8-bit distance channel per slide. Its 512 × 288 projection gate starts at (96, 208). Rotating the slide brings previously hidden material into view. [Full-slide rotation proof](../experiments/slide-look-study/approval/windows-live/slide-motion.md#complete-slide-behind-the-aperture) Two adjacent states occupy a GPU texture array. Each tugged track has a 320 × 180 RGBA16F texture holding the two compact material displacements. The GPU evaluates rigid pose, interpolates the two signed distances, estimates the local source footprint and renders the shared aperture optics into an 8-bit RGBA preview. The FIFO surface requests a maximum frame latency of two. [In-plane motion model and measured liquid/carrier areas](../experiments/slide-look-study/approval/windows-live/slide-motion.md) Playback follows the sampled trajectory forward and backward, repeating every 190/24 seconds. Dragging changes how the selected slide is sampled. The cached fluid solution remains the basis for each new frame.

To reproduce the complete-slide cache, use the geometry exporter and Basilisk solver in their `--world` modes on both slides. Compile the [native cache packer](../experiments/slide-look-study/pack-world-cache.cpp) with CImg and zlib, then run `pack-world-cache SOLVER-OUT NEW-CACHE.gz` on their 96-state directories. For the original cropped [TSSDF001 regression fixture](assets/slide-fields.rg8.gz), use the optical study's lossless FFV1 sequence and the Node.js, FFmpeg and ffprobe converter below. The optics shader comes from the same source as the still and clip proofs. The six additional wheel profiles use the photographic fits in the editor registry. [Geometry](../experiments/slide-look-study/geometry.md) · [Solver](../experiments/slide-look-study/solver.md) · [Field packing](../experiments/slide-look-study/README.md) · [Shared optics](../experiments/slide-look-study/optics.hook) · [Wheel registry](../WebAssets/assets/mathmos-wheels.json)

```sh
node live-wheel/tools/make-field-cache.mjs performance.mkv regenerated-fields.rg8.gz
```

Consult the [material model notes](MATERIAL-MODELS.md), [oil, wax and glass properties](materials/fluids-and-glass.json), and [specialty material measurements](materials/specialty.json) for physical parameters and source conditions.

## Measure all seven wheels

Run from an interactive Windows desktop with the editor visible. Supply a new output basename in an existing, writable directory. This PowerShell command appends a timestamp so repeated runs retain their results.

```powershell
$report = Join-Path $PWD ("wheel-fps-{0:yyyyMMdd-HHmmss}.json" -f (Get-Date))
cargo run --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol -- --benchmark-live $report
```

The benchmark selects the controls through the editor at the 640-pixel preview width cap and sends window-local mouse drags. The first two timed windows exercise rear material tug and rear rigid rotation on Violet / Blue. Six more windows exercise the other wheels using both slides and both actions. Each window has two seconds of warmup followed by twelve measured seconds. Three-second motion recordings precede the warmups for the first two windows. The JSON reports the actual optical and surface sizes, FPS, p50/p95/p99 intervals, skipped frames and applied mouse moves. FPS measures the cadence of successful FIFO present returns on the host. This metric includes display pacing. It does not measure isolated shader time or physical input-to-screen latency.

Before timing, the benchmark captures the same fixed-time frame twice, exercises both in-plane gestures on a disposable track, and counts pixels that change by at least three RGB codes in any channel. It writes five PPMs beside the JSON for the baseline, tug, rotation and two focal endpoints. The timed windows run on the performance track without GPU readbacks. [Benchmark implementation](../src-tauri/src/live/windows/benchmark.rs)

`the-sacred-swirls.exe --capture-live C:\captures\fresh-directory` records each successfully presented native optical frame for eight seconds. Supply a new directory name under an existing parent. It saves P6 RGB PPMs, actual frame intervals in `frames.ffconcat`, and configuration changes in `capture.json`. The live editor continues after the recording. Readback and disk I/O affect capture cadence. Use the separate benchmark for performance measurements. [Capture implementation](../src-tauri/src/live/windows/capture.rs) · [Windows measurements and motion clips](../experiments/slide-look-study/approval/windows-live/README.md)

For pixel and pipeline regression testing on a development machine, set `VK_DRIVER_FILES` to a software Vulkan ICD manifest. Run `cargo test --locked --manifest-path live-wheel/Cargo.toml --features wgpu/vulkan-portability --lib render_wheels_on_software_vulkan -- --ignored`. The test requires every enumerated adapter to report CPU, renders all seven wheels offscreen, and checks track isolation and both focus endpoints.

The first physical Windows benchmark measured 39.92–40.00 FPS over all seven wheels on the GTX 1650 Ti Max-Q at 640 × 360 optical resolution. The run used two fixed-gap slides and applied a rear-slide mouse drag throughout each wheel. [Per-wheel timings and artifacts](../experiments/slide-look-study/approval/windows-live/README.md)

Live replay eases the turnaround over the first and last eight source intervals. This avoids an instantaneous velocity reversal while keeping the original endpoints and duration. The single-frame contrast measurement and the owner's separate report at the edge of a repeatedly swirled region are tracked in the [steady-motion study](../experiments/slide-look-study/approval/windows-live/README.md#steady-motion-capture-on-the-windows-gpu).

The finite-pixel optical comparison uses a per-frame texture of both slides' optical properties and the same 24 paired pupil samples. It measures transmission at subpixel positions along deformed edges and samples material depth and rim support at the ray intersections. A narrow rotated rear film broadened and lost peak contrast through focus while retaining 99.99% of its measured integrated linear attenuation in the isolated CPU Vulkan fixture. [Transmission, finite-footprint measurements and reproduction](../experiments/slide-look-study/approval/footprint/README.md)

The selected finite-pixel model also renders the stationary image. On a frozen 320 × 180 Violet / Blue frame, the first 0.00000001 radian of rotation changed zero output codes. The previous pose-dependent selection changed 4,795 pixels by three or more codes, with a maximum difference of 31.

The native program output supports optional timestamped recursive video feedback. The Windows projector uses the same GPU and an independent audience delay with monotonic presentation deadlines. [Split-screen clip, transport behavior, buffer budgets and test commands](../experiments/slide-look-study/approval/feedback/README.md)
