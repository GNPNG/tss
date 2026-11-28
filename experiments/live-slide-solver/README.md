# Evolving sealed oil slides

A single-color oil layer runs one persistent Basilisk worker. Older paired tracks run one worker for each of their two sealed cells. Each worker initializes a closed square cell from its seeded union of dyed drops and a divergence-free vortex. Eight large pools and sixteen smaller inclusions are sampled per cell. The signed seed function is evaluated during initialization only. Each subsequent step uses Basilisk's geometric VOF advection, incompressible Navier–Stokes pressure projection, viscosity and surface tension. Only each cell's coverage crosses the process boundary.

The interactive tangential drag uses a depth-averaged Couette term. For local plate velocity **U**, gap h, viscosity μ, density ρ and mean fluid velocity **u**, the added acceleration is 12μ/(ρh²) × (w**U**/2 − **u**). The moving contact weight is w = (1 − r²/R²)² for r < R, zero elsewhere, with R = 0.16 slide widths. Positive drag moves the plate toward the pointer displacement. Releasing it sets **U** to zero and the fluid retains its velocity. The interactive materials use nondimensional constants.

Normal pressure prescribes local glass deflection. For load P, squared relative radius s = r²/R² and s < 1, the continuum gap ratio is h/h₀ = 1 − 0.65 P (1 − s)² (1 − 4s). The center thins and an annulus accepts the displaced volume. The worker differences the integrated depth flux 0.325 P **r** (1 − s)³ at each shared face between two contact configurations. Basilisk's piecewise-linear interface reconstruction determines the dyed and carrier portions of each directional flux. Each substep advances the phase volumes and cell capacity together. Closed wall flux is zero. The worker uses the resulting gap in its cell-volume metric, face metric, pressure projection and viscous wall drag as the flow continues. The quadtree stays uniformly refined at the configured live level during contact and resumes adaptive refinement after release.

The prescribed glass shape is an artistic constitutive assumption. Microscopic wetting, three-dimensional meniscus dynamics and drainage of the film between coalescing droplets remain outside this model.

On the 64² grid, a quarter press on a dyed boundary changed 42 coverage samples at fixed simulation time. A full press changed 49 samples with 4.2 times the integrated absolute coverage change. The oil and carrier volume differences during the paused press, translation and release sequence were below 10⁻¹⁵ relative. During 30 evolving steps with a press and release, the maximum relative volume drifts were 6.6 and 3.5 parts per million, respectively. The independently run second cell matched its control at every tick. [32² measurements](evidence/squeeze-32.txt) · [64² measurements](evidence/squeeze-64.txt)

## Build and repeat the measurements

Use the [project's matching Basilisk build](../basilisk-study/README.md) and the [CImg 3.6.0 source revision](https://github.com/GreycLab/CImg/tree/01230c15cfe23d5ffd9b1d1ba05aefdbcac6ab00). CImg.h SHA-256 is `d769ec34013eba8f19bdc7eb42f4f416159b1ca3840a2b4a2e4aa1a14d9fa85d`. The interactive renderer links its C++ optics bridge statically. A plain build of the Rust solver client needs no CImg header. From the TSS repository root, choose a new output executable.

```sh
mkdir -p target/live-slide-solver
export BASILISK="$PWD/experiments/basilisk-study/vendor/patched/basilisk/src"
export CIMG_INCLUDE=/absolute/directory/containing/CImg.h
export LC_ALL=C
# Intel Mac toolchain used for these measurements
export CC=/usr/bin/clang CXX=/usr/bin/clang++
experiments/live-slide-solver/build-worker.sh target/live-slide-solver/tss-live-solver

cargo +1.88.0 build --release --jobs 1 --locked \
  --manifest-path experiments/live-slide-solver/Cargo.toml --features cimg-optics \
  --examples --target-dir target/live-slide-solver/host
target/live-slide-solver/host/release/examples/check target/live-slide-solver/tss-live-solver 5
target/live-slide-solver/host/release/examples/check target/live-slide-solver/tss-live-solver 6
target/live-slide-solver/host/release/examples/squeeze_flow target/live-slide-solver/tss-live-solver 6 target/live-slide-solver/pressure-masks
target/live-slide-solver/host/release/examples/optical_pair \
  target/live-slide-solver/tss-live-solver 5 176 48
```

## Coordinate and integration contract

`STEP x y dx dy drive press_x press_y radius load` sets the normal pressure for the interval and accepts the same tangential input as the five-argument form. Its last four fields use normalized whole-slide units, top-left origin and a load from zero to one. The five-argument form advances with released normal pressure. `PRESS press_x press_y radius load` applies just the squeeze transport at the current fluid time and returns the next numbered frame. This lets a performer press, move and release while flow speed is zero. The header's integrated phase fraction is weighted by local gap depth. Setting `TSS_LIVE_VOLUME_AUDIT=1` adds a diagnostic stderr record per frame with oil volume, carrier volume, total capacity, and minimum and maximum gap ratios.

Start `tss-live-solver --live violet|cyan SEED [LEVEL]`. LEVEL 5–8 selects a maximum grid of 32, 64, 128 or 256 cells across. Its default is 5. At 32, the quadtree minimum and maximum coincide. Time zero is returned on startup. For each subsequent interval write `STEP x y dx dy drive\n`. XY and displacement are normalized to the entire square cell. X increases rightward, Y downward. X and Y range over [0,1], dx and dy over [−1,1], drive over [0,1]. The command applies a wall velocity of drive × displacement / (1/24) slide widths per simulation time unit for exactly one 1/24 interval. A contact-free step uses zero drive. Basilisk may take several internal steps in each interval. `QUIT\n` or EOF ends the worker. Output is a `TSSVOF1 tick time width height phase_fraction\n` ASCII header followed immediately by width × height row-major R8 coverage bytes. The phase fraction in the header is the integrated numerical field before rasterization. The process writes its normal Basilisk timing footer at shutdown.

`Atlas::pack` uses the same [CImg reconstruction, Euclidean signed distance, half-texel offset and smoothing](../slide-look-study/optical-distance.hpp) as the existing wheel packer. Inside oil is positive. The optical byte is round(255 × clamp(0.5 + d × 720 / (gate height × 96), 0, 1)) for signed distance d in atlas texels. R carries the rear film, G the front, row zero is at the top. A 176² atlas has the centered view [24,52,128,72], proportional to the prior [96,208,512,288] on 704². `optical_pair` can write a new uncompressed TSSDF002 stream as its optional last argument.

With the `live-solver` feature, `LiveWheel::set_single_cell_solver(track_id, executable, seed, rate)` starts one cell. Its signed-distance field occupies R in the native optical texture, with G cleared to the fully outside value. The paired API `set_live_solver` fills both channels from separate cells. The presenter retains the two completed RG8 fields that bracket the displayed phase and interpolates them through the standard projector optics, per-film poses, color and focus. It requests at most three solver ticks beyond the lower endpoint. If the worker falls behind, the presenter stops at its newest completed field and discards the missed wheel time. Rate is simulation seconds per forward wheel-clock second. Zero holds the exact displayed fraction, including when an in-flight solve finishes. Resuming keeps that phase on the first draw and advances it from the next wheel-clock interval. The physical source reads the track’s independent rear-film clock. A paused or seeking song leaves it advancing. Solvers report starting, ready, or failed through `live_solver_status`. Removing the source terminates its workers.

## Results on the Intel Mac

At the default 32² grid, three independently seeded single-cell runs each spawned exactly one worker and advanced eight steps. A heavy tug on the middle layer changed 357 of its 176² signed-distance samples relative to its unforced control on the eighth step. Both other layers matched their respective controls byte for byte. The green channel stayed clear throughout all six runs. To repeat, set `TSS_LIVE_SOLVER` to the compiled Basilisk executable and `CIMG_INCLUDE` as above, then run `cargo +1.88.0 test --locked --features live-solver --manifest-path live-wheel/Cargo.toml independent_cells_evolve_and_a_heavy_tug_changes_only_its_cell -- --ignored --nocapture`. This test exercises CPU simulation and optical reconstruction.

Over 18 ticks, a 6-tick front-cell contact changed 283 of 4,096 coverage samples at release on the 64² grid. Ten ticks later, 418 samples differed from the unforced control. The rear film matched its independent unforced control byte for byte at every tick. Maximum measured relative dyed-volume drift across driven and control copies was 2.93 × 10⁻⁶ front and 3.03 × 10⁻⁶ rear. The 32² check passed the same conservation, independence, and post-release evolution assertions. [32² measurements](evidence/mass-and-tug-32.txt) · [64² measurements](evidence/mass-and-tug-64.txt)

Two 32² solvers advanced serially and converted to a single 176² signed-distance pair in 20.79 ms median, 36.68 ms p95, 40.66 ms maximum over 48 ticks. These measurements cover CPU simulation, IPC and CImg. Projector rendering and display synchronization are outside that interval. The tuned rates remain hardware-, seed- and scene-dependent. [Timing measurements](evidence/optical-pair-32-to-176.txt)

The 60-frame, 384 × 216 Metal capture uses the AMD Radeon Pro 5300M, the procedural seed 83724, a front-cell tug and release, and a rear-to-front focus pull. Playback is 24 fps. The measured draw/readback procedure waited for the requested solver tick and serialized GPU readbacks. The 32² field produces coarse contours. [Captured frame](evidence/focus-pull.png) · [2.5-second video](evidence/focus-pull.mp4)

The existing violet PGM study was also compared with its parent implementation at grid level 7. Default and SI-material runs each produced byte-identical initial and first advanced 1024 × 576 phase images. The SI test used a 50 mm scene width, 50 mm/s reference speed, 2.5 mm gap, densities 950 and 1000 kg/m³, viscosities 0.019 and 0.001 Pa·s, and 0.035 N/m surface tension.

The Intel macOS review bundle rendered the default seeded slide in its visible editor on the AMD Radeon Pro 5300M through Metal. At 640 × 360, the short startup capture recorded 27 presentations, 25 fluid steps, one rear-slide mouse drag, 23.99 FPS and a 45.51 ms p95 frame interval. This is a startup sample with one visual track and two fluid cells. [Native editor frame](evidence/mac-editor.png) · [Presentation counters](evidence/mac-editor.json)

A second package run automatically brought its own verification window forward. It recorded 24 presentations, 24 simulated steps, one local drag, 21.58 FPS and a 50.30 ms p95 interval at the same resolution. Both timings cover startup. [Second run](evidence/mac-editor-relaunch.json)

At fixed 2.5× magnification (Detail Scale 0.4), eight consecutive quarter-tick steps changed 4,871–5,593 of the 82,944 pixels in the 384 × 216 Metal projection on the AMD Radeon Pro 5300M. Both cells used track seed 83724. The test performed a front-cell tug and release and completed its focus pull before measuring the magnified motion. A pause between solver ticks reproduced the entire RGBA image after 30 seconds of supplied wheel-clock time. The first resumed frame still matched, and another quarter-tick changed the image. The test waited for solved fields and read back each frame, so these are image-continuity measurements. The GPU-free timing regressions exercised 0×, 0.25×, 0.5×, 1×, 1.5× and 2× at 60 presentations per wheel-clock second, including a delayed worker and a pause with a solve in flight. [Timing regressions and Metal capture test](../../live-wheel/src/evolving.rs)

The windowed Mac verifier (`--verify-mac-preview`) needs an unlocked, visible desktop. A locked console suppresses presentation even when WebKit finishes loading the editor. The offscreen Metal readbacks above exercise the film sampler without opening an editor window. Windowed FPS requires a separate visible run.
