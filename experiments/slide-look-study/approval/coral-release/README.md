# Coral release on two sealed slides

The [release and focus pull](coral-release.mp4) shows the Violet / Blue wheel with Coral release selected. Watch the cyan neck just left of center. The native cache first separates it from the bank at 2.625 s. A second neck farther left opens by 3.125 s. The violet rear film continues its own motion behind both. [Still at 3.21 s](coral-release.png) · [Solver geometry, volume and connectivity measurements](../../../coral-release/)

The front stays in focus throughout the simulated release. The simulation reaches its last sample at 3.958 s. From 4 to 4.85 s the projector shifts focus to the rear film, holds, then returns to the front from 5.5 to 6.5 s. Changing focus on this fixed geometry alters 45,177 of 230,400 pixels by three or more RGB codes. Restoring front focus reproduces the earlier front-focused image byte for byte.

The [synchronized material tug](front-tug-comparison.mp4) shows the same cyan neck at twice its native size. The left view follows the recorded fluid solve. The right adds one drag with **Front slide**, **Material tug**, **Light**. Contact starts at 1.5 s at viewport coordinates (0.46, 0.58) and ends at 2.5 s at (0.59, 0.45). The front film keeps the deformation after release. The solver supplies the neck separation in both views. The drag changes the visible shape and trajectory through the live material displacement. Both tracks use identical time, palette, rear pose and front focus. Their pixels match before contact. At the end of the drag 2,704 pixels differ by at least three RGB codes across the full 640 × 360 frame. [Per-frame comparison](front-tug.json)

## Render measurements

The AMD Radeon Pro 5300M rendered through Metal. The 640 × 360 clip has 168 native frames at 24 fps, encoded as seven seconds of 8-bit Rec.709 H.264. The still uses the original sRGB readback. Mean optical render plus synchronous readback took 7.13 ms per frame, with median 7.21 ms and p95 9.34 ms. This timing excludes video encoding and window presentation. The different motion sources changed 201,252 pixels at the same initial time. Switching back to the ordinary wheel restored the original pixels exactly. [Hardware metadata and frame timings](capture.json)

The comparison uses the same native GPU at 640 × 360 per track. Its viewing copy crops the same 192 × 144 region at (202, 133) in each view and enlarges it 2× for inspection. Labels sit above the image. The underlying readbacks determine all comparison counts.

## Reproduce the native frames

Run from the repository root with Rust 1.88 and the build prerequisites for the live wheel. Set the adapter substring to the discrete GPU you intend to use. Supply a new capture directory for each run. The output contains full-range sRGB RGBA8 frames in top-down row order. The harness uses one synchronous readback at a time.

```sh
CC=/usr/bin/clang CXX=/usr/bin/clang++ TSS_LIVE_ADAPTER='Radeon Pro 5300M' \
  TSS_CORAL_CAPTURE_DIR="$PWD/coral-focus-capture" \
  cargo test --locked --manifest-path live-wheel/Cargo.toml --lib capture_coral_release -- --ignored --test-threads=1

CC=/usr/bin/clang CXX=/usr/bin/clang++ TSS_LIVE_ADAPTER='Radeon Pro 5300M' \
  TSS_CORAL_TUG_COMPARISON=1 TSS_CORAL_CAPTURE_DIR="$PWD/coral-tug-capture" \
  cargo test --locked --manifest-path live-wheel/Cargo.toml --lib capture_coral_release -- --ignored --test-threads=1
```
