# Live Mathmos wheels

From the repository root, build and launch the desktop editor in PowerShell. Building requires the project's Rust 1.88 toolchain, the Windows Tauri prerequisites and `glslc` from the Vulkan SDK (`VULKAN_SDK`, `PATH`, or the compiler path in `GLSLC`). The executable uses the installed Vulkan graphics driver. CUDA is unnecessary. [Windows Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#windows)

```powershell
cargo run --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
```

The Windows host enumerates hardware adapters that can present to the editor and prefers a discrete GPU. To select one, set `$env:TSS_LIVE_ADAPTER = 'GTX 970'` before launching. The filter accepts a case-insensitive substring, including names of integrated and non-NVIDIA GPUs. A GTX 970 with a suitable Vulkan driver can be tried through this same build. The live readout identifies the adapter and reports its measured cadence.

## macOS editor preview

Build the desktop editor with Rust 1.88, the Xcode command line tools and `glslc` on the compiler search path. The Mac preview opens Violet / Blue in a Metal-backed AppKit view. The **Live selected wheel** checkbox switches the selected track between its live wheel and the existing study controls. The width control starts at 640 pixels. The native readout identifies the active adapter, render size and measured frame cadence.

```sh
cargo +1.88.0 run --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
```

The host prefers a discrete presenting adapter. To select the Radeon on a dual-GPU Mac, launch with `TSS_LIVE_ADAPTER='Radeon'`. For a bounded inspection, append `-- --verify-mac-preview /path/to/new-directory` to the command above. The editor records five presentations, dispatches a local AppKit drag into the preview, writes the wheel image and measurements, and exits. It also requests an image of its own window using macOS screen capture.

## Physical oil selection

With **Wheel flow** selected, choose **Rear film oil** and **Front film oil** independently. Each film offers **Original wheel**, **20 cSt PDMS** and **1,000 cSt PDMS**. The PDMS grades refer to silicone oil in water. The wheel preset supplies the dyes, track opacity sets the contribution to the composite, and **Material response** sets the response to a manual tug.

**Wheel with coral beads** uses the selected rear oil and the prescribed front bead release. **Coral release** supplies both trajectories. Returning to **Wheel flow** restores the saved selections. Projects created before these controls use **Original wheel** on both films.

## Performance controls

1. Select a visible liquid track and choose a preset under **Mathmos oil wheels**. **Live selected wheel** opens that track in the native preview. Windows and macOS start with Violet / Blue.
2. Choose **Rear slide** or **Front slide** under **Mouse target**. Choose **Material tug** or **Rotate slide** under **Mouse action**, then hold the left button and drag on the native image. A tug pulls the selected liquid locally. **Material response** sets **Heavy**, **Medium**, or **Light** for that slide. Rotation turns the whole selected slide about the optical center. Releasing the button holds the pose. Each track keeps two independent slide poses.
3. Select **640 × 360** or **960 × 540** under **Live resolution**. These cap the optical render width. A smaller viewport renders at its own width. The last pass scales the optical image to the stage.
4. Move **Projector focus** between the rear and front planes. It overrides the default fitted focus across both slides and tracks. **Restore wheel focus** returns to the spatially varying focus used in the wheel study. **View study** opens the recorded reference. The live readout under the stage shows adapter, optical resolution, FPS and p95 frame interval.

Plate pressure acts independently on each sealed slide. A press reduces the central gap, displaces liquid sideways and accommodates the displaced volume in a peripheral annulus. Release restores the underlying tug and rotation. [Native press/release movie, geometry and volume checks](../experiments/slide-look-study/approval/pressure/README.md)

The native viewport shows the two sealed slides on the selected track. Its fluid trajectory advances on a performance clock. The gesture deck can replay live inputs against the song.

## Artwork, footage and rotoscopes

On Windows, **Add artwork** imports an image, animated GIF or video as its own timeline layer. Place the layer above a wheel to project the artwork over the liquid, or below a wheel to transmit its color through both films. Source aspect ratio and alpha are preserved. The GPU blends in linear light. The editor preview and delayed projector use the prepared preview frames. Native movie exports use the full-size source and sample its timestamps at each movie frame.

**Rainbow silhouette** runs a full hue cycle through the artwork's alpha. Set **Cycles per second** to zero for a fixed spectrum. **Star-wipe tunnel** projects a sequence of expanding five-point stars. **Tunnel speed** specifies octaves per second. **Bass acceleration** increases the expansion rate with bass amplitude. Seeking returns the tunnel to its integrated phase at the requested song time.

**Source offset**, **Playback speed** and **Loop footage** set each source clock. Disabling looping holds the last frame. Cropped and repeated timeline intervals retain their local media start and speed. The same interval fades control the artwork and its use as a matte.

**Apply chroma key** removes the selected background color before silhouette or gradient treatment. **Mask A** and **Mask B** take alpha from artwork, or the combined dyed area of a wheel's exposed films. A hidden source used as a matte still runs. Intersection selects the lesser coverage, union the greater, subtraction keeps the positive difference, and exclusive-or takes the absolute difference. The source matte includes its own key, shape, opacity and fades. **Bass bounce**, **Midrange distortion** and **Treble blur** use the song's analyzed bands.

## Coral release

**Projected films** controls which dyes reach the screen for this wheel track. Select **Front only** to inspect a pinching neck, **Rear only** to inspect the moving fold, or **Clear glass** to see the carrier and lamp. **Both films** restores the overlap at the current time with each slide's pose intact. The same selection applies to the delayed projector and to the full duration of a native export.

**Wheel with coral beads** pairs the original wheel's rear film with a separate front film containing smaller oil buds. Pick **Violet / Blue** for the violet rear and cyan front shown in the review. **Replay front beads** restarts the front film while the rear stays at its current phase. Once the last front droplet has detached, the rear keeps moving behind it. **Current pose and film position** preserves both film times for export. **Neutral pose and film start** takes both back to zero.

**Slide motion → Coral release** selects a second pair of independently solved films. Use any of the seven color wheels. The foreground starts with buds attached to a bank of oil. The rear carries broad folds that remain visible through the intervening glass. Move **Projector focus** toward the front to follow a neck as it thins, then toward the rear to see the folds through the newly detached drops.

The finite solve has 96 states at 24 Hz. Playback holds the last state at 95/24 seconds. **Replay release live** restarts the selected track in the editor and on the delayed projector, retaining its slide poses. Material tug, rotation and compression change the projection during playback. The timing and sites of pinch-off come from the stored solve. The native exporter starts **Automatic** coral exports at the beginning of the release with neutral slide poses. **Current pose and film position** starts the export at the displayed point in the take. Manual live restarts are outside the recorded gesture schedule.

The first selection prepares the entire coral cache on the CPU. Five fresh processes on an Intel Core i7-9750H Mac took 232–284 ms (median 236 ms) to decode and register the cache using the release build of the production field reader. Filesystem pages were allowed to stay warm. This delay occurs before live frame timing. Audition the preset once before a performance to keep both 90.75 MiB field sequences resident in host memory. Subsequent switches reuse the same two GPU texture slots. To time the reader separately, run `cargo test --release --locked --manifest-path live-wheel/Cargo.toml coral_advances_forward_and_holds_its_detached_geometry -- --nocapture`.

To measure this preset on a Windows GPU, use the existing editor benchmark with `--benchmark-live NEW-REPORT.json --coral-trial`. The trial requests three releases during a twelve-second front-slide tug and checks that the editor applied all three. Its frozen-frame check compares the start, middle, end and held state before collecting presentation timings.

## Stack native wheels

On Windows, select a Mathmos wheel, add a visual layer and choose another wheel for it. The upper timeline row sits nearest the audience. Select either wheel to tug, rotate or press one of its films while the other wheel continues. Use the layer visibility button, opacity slider, reorder arrows and **Projected films** to change what reaches the screen. Preview, the delayed audience window and **Export wheel stack in 4K** use the same ordered light path. Scheduled gestures retain their target wheel in the movie.

Each wheel samples its own pair of film clocks and poses. Its existing optical shader attenuates the incident linear RGB image and adds that wheel’s fitted veiling light. A single lamp illuminates the stack. With manual focus, the ordered film planes span the existing focus travel. Each wheel integrates its own pupil footprint before transmitting the result to the next wheel. This screen-space approximation does not trace refracted rays across wheels. A single opaque wheel keeps its original optical path. The native stack accepts Mathmos wheel tracks.

## Export still plates

Select a live wheel in the Windows editor, expand **Selected wheel · production stills**, choose a format and color space, then click **Export selected wheel · 16-bit stills** and choose the containing folder. The editor captures the selected track's current rear and front positions, focus, gesture deformation and film visibility after the chooser closes. With multiple wheels visible, this control projects the selected wheel alone under its own lamp at full optical density. The stack movie uses the shared optical path and layer opacities. Audio is optional. Each export creates a new folder. The wheel clocks pause during rendering and encoding.

**Preview current framing** samples the selected wheel for the framing guide. The output derives from the full-resolution optical shader. The DSP cover fills 4000 × 4000 in sRGB. The vertical crop fills 2160 × 3840 in sRGB or Rec.709. Place title and sigil in the marked 15–28% band and reserve the lower 20% for platform UI. The visualizer offers Rec.709 or Display P3 and an optional horizon guide, initially 1339 px from the top. The exported guide references the clean master. Guide marks stay off the master.

The 10800 × 2160 panorama offers Display P3 or sRGB. It includes five 2160 × 2160 panels named EPNI-006 through EPNI-010. Each panel is an exact crop of the master. The outer edges join through a 216 px smooth blend of genuine overscan in linear light. Every layout crops the native 16:9 wheel at fixed optical aspect and embeds the matching ICC profile and PNG cICP tag.

Open **projection.png** for the 3840 × 2160 optical still. For compositing, stack **BG.png**, **MID.png** and **FG.png** in that order from back to front. Use Normal/Over blending in linear light with straight alpha. Interpret the PNGs using their embedded color profile. The still captures the optics before electronic feedback.

BG contains the lamp and clear carrier. MID introduces the visible rear film. FG adds the visible front film, including its refraction through the rear film, joint optical density and veiling light. These registered layers reconstruct the captured projection to file quantization precision. Changing their relative positions in a compositor creates a new image. Reposition the physical films in the editor to render the interaction at a new alignment.

**rear-solo.png** and **front-solo.png** each show one film projected under the lamp. Both are opaque. Combining the two solo exposures omits the common pupil rays, refraction and flare. **plates.json** identifies the wheel, clocks, focus, visibility, pixel encoding and composition order. [Rec.709 verification](../experiments/rec709-plate/README.md)

## Export the selected wheel

Open a local song, choose the live wheel, then select **Export selected wheel in 4K**. Choose a new MP4 filename. Export finishes an open gesture recording before reading the schedule. A take recorded while the song was paused can be assigned a trigger in the gesture deck. The current wheel material and focus apply throughout. Enabled gestures scheduled at a recording position, audio motif or lyric phrase replay in song order at 30 fps. **Export starting pose** offers three choices.

- **Automatic** starts both films at zero with neutral slide poses for **Coral release**, **Wheel with coral beads**, and any track with scheduled gestures. Unscheduled **Wheel flow** starts from its current poses and visible film position.
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
