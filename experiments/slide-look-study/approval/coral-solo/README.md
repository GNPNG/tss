# Soloing the coral wheel films

The left panel shows the rear violet film, the center shows the front cyan film, and the right shows the transmitted projection through both. Each uses the same moving geometry and the same focus, 90% toward the front. The 96 frames cover four seconds at 24 fps. [Watch the three views](coral-solo.mp4) · [Still at 2.5 seconds](coral-solo.png)

The Radeon Pro 5300M rendered each panel through Metal at 960 × 540. All 199,065,600 raw bytes in the combined panel matched the corresponding first 96 frames of the previously approved coral projection. The viewing copy is H.264 Rec.709. [Capture settings and hashes](capture.json) · [Original projection](../coral-wheel/coral-wheel.mp4)

On Violet / Blue and Blue / Red, the smaller 320 × 180 isolation test changed the hidden film's time, tug, rotation and pressure while holding the visible film still. Zero output pixels changed across both solo selections and both finite-pixel settings. Native preview and movie readbacks matched for each selection. [Hardware test](../../../../live-wheel/src/coral_solo.rs)

To repeat the native capture on the Mac, provide a new directory for the raw output and the name of the available discrete Metal adapter.

```sh
CC=/usr/bin/clang CXX=/usr/bin/clang++ RUSTUP_TOOLCHAIN=1.88.0 \
TSS_LIVE_ADAPTER='Radeon Pro 5300M' TSS_CORAL_CAPTURE_DIR="$PWD/coral-solo-capture" \
cargo test --locked --manifest-path live-wheel/Cargo.toml --lib \
  coral_capture::solo::capture_coral_solos -- --exact --ignored --test-threads=1
```
