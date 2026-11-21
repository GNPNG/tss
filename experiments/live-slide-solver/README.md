# Evolving sealed oil slides

A track owns two persistent Basilisk workers. Each initializes a closed square cell from its own seeded union of dyed drops and its own divergence-free vortex. Eight large pools and sixteen smaller inclusions are sampled per cell. The signed seed function is evaluated during initialization only. Each subsequent step uses Basilisk's geometric VOF advection, incompressible Navier–Stokes pressure projection, viscosity and surface tension. Only each cell's coverage crosses the process boundary.

The interactive tangential drag uses a depth-averaged Couette term. For local plate velocity **U**, gap h, viscosity μ, density ρ and mean fluid velocity **u**, the added acceleration is 12μ/(ρh²) × (w**U**/2 − **u**). The moving contact weight is w = (1 − r²/R²)² for r < R, zero elsewhere, with R = 0.16 slide widths. Positive drag moves the plate toward the pointer displacement. Releasing it sets **U** to zero and the fluid retains its velocity. The numerical model has a fixed gap and artistic nondimensional constants. Normal pressure currently uses the editor's volume-conserving material map. Physical squeeze flow, wetting and film drainage before coalescence remain open.

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
target/live-slide-solver/host/release/examples/optical_pair \
  target/live-slide-solver/tss-live-solver 5 176 48
```

## Coordinate and integration contract

Start `tss-live-solver --live violet|cyan SEED [LEVEL]`. LEVEL 5–8 selects a maximum grid of 32, 64, 128 or 256 cells across. Its default is 5. At 32, the quadtree minimum and maximum coincide. Time zero is returned on startup. For each subsequent interval write `STEP x y dx dy drive\n`. XY and displacement are normalized to the entire square cell. X increases rightward, Y downward. X and Y range over [0,1], dx and dy over [−1,1], drive over [0,1]. The command applies a wall velocity of drive × displacement / (1/24) slide widths per simulation time unit for exactly one 1/24 interval. A contact-free step uses zero drive. Basilisk may take several internal steps in each interval. `QUIT\n` or EOF ends the worker. Output is a `TSSVOF1 tick time width height phase_fraction\n` ASCII header followed immediately by width × height row-major R8 coverage bytes. The phase fraction in the header is the integrated numerical field before rasterization. The process writes its normal Basilisk timing footer at shutdown.

`Atlas::pack` uses the same [CImg reconstruction, Euclidean signed distance, half-texel offset and smoothing](../slide-look-study/optical-distance.hpp) as the existing wheel packer. Inside oil is positive. The optical byte is round(255 × clamp(0.5 + d × 720 / (gate height × 96), 0, 1)) for signed distance d in atlas texels. R carries the rear film, G the front, row zero is at the top. A 176² atlas has the centered view [24,52,128,72], proportional to the prior [96,208,512,288] on 704². `optical_pair` can write a new uncompressed TSSDF002 stream as its optional last argument.

With the `live-solver` feature, `LiveWheel::set_live_solver(track_id, executable, seed, rate)` starts a CPU worker thread that publishes successive solved fields. The presenter retains the two completed RG8 fields that bracket the displayed phase and interpolates them through the standard projector optics, per-film poses, color and focus. It requests at most three solver ticks beyond the lower endpoint. If the worker falls behind, the presenter stops at its newest completed field and discards the missed wheel time. Rate is simulation seconds per forward wheel-clock second. Zero holds the exact displayed fraction, including when an in-flight solve finishes. Resuming keeps that phase on the first draw and advances it from the next wheel-clock interval. The physical source reads the track’s independent rear-film clock. A paused or seeking song leaves it advancing. Solvers report starting, ready, or failed through `live_solver_status`. Source removal cancels both children.

## Results on the Intel Mac

Over 18 ticks, a 6-tick front-cell contact changed 283 of 4,096 coverage samples at release on the 64² grid. Ten ticks later, 418 samples differed from the unforced control. The rear film matched its independent unforced control byte for byte at every tick. Maximum measured relative dyed-volume drift across driven and control copies was 2.93 × 10⁻⁶ front and 3.03 × 10⁻⁶ rear. The 32² check passed the same conservation, independence, and post-release evolution assertions. [32² measurements](evidence/mass-and-tug-32.txt) · [64² measurements](evidence/mass-and-tug-64.txt)

Two 32² solvers advanced serially and converted to a single 176² signed-distance pair in 20.79 ms median, 36.68 ms p95, 40.66 ms maximum over 48 ticks. These measurements cover CPU simulation, IPC and CImg. Projector rendering and display synchronization are outside that interval. The tuned rates remain hardware-, seed- and scene-dependent. [Timing measurements](evidence/optical-pair-32-to-176.txt)

The 60-frame, 384 × 216 Metal capture uses the AMD Radeon Pro 5300M, the procedural seed 83724, a front-cell tug and release, and a rear-to-front focus pull. Playback is 24 fps. The measured draw/readback procedure waited for the requested solver tick and serialized GPU readbacks. The 32² field produces coarse contours. [Captured frame](evidence/focus-pull.png) · [2.5-second video](evidence/focus-pull.mp4)

The existing violet PGM study was also compared with its parent implementation at grid level 7. Default and SI-material runs each produced byte-identical initial and first advanced 1024 × 576 phase images. The SI test used a 50 mm scene width, 50 mm/s reference speed, 2.5 mm gap, densities 950 and 1000 kg/m³, viscosities 0.019 and 0.001 Pa·s, and 0.035 N/m surface tension.

The Intel macOS review bundle rendered the default seeded slide in its visible editor on the AMD Radeon Pro 5300M through Metal. At 640 × 360, the short startup capture recorded 27 presentations, 25 fluid steps, one rear-slide mouse drag, 23.99 FPS and a 45.51 ms p95 frame interval. This is a startup sample with one visual track and two fluid cells. [Native editor frame](evidence/mac-editor.png) · [Presentation counters](evidence/mac-editor.json)

A second package run automatically brought its own verification window forward. It recorded 24 presentations, 24 simulated steps, one local drag, 21.58 FPS and a 50.30 ms p95 interval at the same resolution. Both timings cover startup. [Second run](evidence/mac-editor-relaunch.json)

At fixed 2.5× magnification (Detail Scale 0.4), eight consecutive quarter-tick steps changed 4,871–5,593 of the 82,944 pixels in the 384 × 216 Metal projection on the AMD Radeon Pro 5300M. Both cells used track seed 83724. The test performed a front-cell tug and release and completed its focus pull before measuring the magnified motion. A pause between solver ticks reproduced the entire RGBA image after 30 seconds of supplied wheel-clock time. The first resumed frame still matched, and another quarter-tick changed the image. The test waited for solved fields and read back each frame, so these are image-continuity measurements. The GPU-free timing regressions exercised 0×, 0.25×, 0.5×, 1×, 1.5× and 2× at 60 presentations per wheel-clock second, including a delayed worker and a pause with a solve in flight. [Timing regressions and Metal capture test](../../live-wheel/src/evolving.rs)
