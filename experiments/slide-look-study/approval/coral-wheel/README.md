# Wheel with coral beads

The [six-second projection](coral-wheel.mp4) pairs the Violet / Blue wheel’s original rear film with the small-drop front slide. Both start at zero. Watch the narrow necks along the upper edge of the blue bank as droplets separate and drift. The front reaches its last recorded state at 3.958 seconds. The rear wheel continues through the end of the clip. Projector focus stays at 90% toward the front.

The [half-speed close-up](coral-wheel-detail-half-speed.mp4) follows the same six seconds, enlarged 2× from a 480 × 270 rectangle beginning at (108, 254) in the full 960 × 540 frame. Each source frame appears once at 12 fps. Several necks around the center let go between 0.875 and 1.583 seconds in the full projection, or 1.75–3.17 seconds in the close-up. Watch the small cyan beads pass over the pale opening between the violet shapes. The [full-gate still at 2.5 seconds](coral-wheel.png) uses the original sRGB samples. [Optical-cache separation measurements](../../../coral-release/small-drops/optical-events.csv)

## Hardware capture

An AMD Radeon Pro 5300M rendered the 144 native frames through Metal with the Windows editor’s finite-pixel optical sampling. Both H.264 viewing copies use 8-bit, limited-range Rec.709. Mean render plus synchronous readback took 27.96 ms, with a median of 32.60 ms and a 95th percentile of 39.54 ms. These measurements end before video encoding and exclude presentation to a window.

The red and green distance channels of the 512 × 288 projected field were read before optical compositing. At rear times 0, 1.137, 3.02 and 6.25 seconds, every rear sample matched the corresponding ordinary wheel sample. Advancing the rear while the front held changed 91,900 rear samples and zero front samples. Replaying the front at a fixed rear time changed 102,615 front samples and zero rear samples. At rear time 2.635 and front time 0.75 seconds, native preview and movie readbacks at 960 × 540 were byte-identical. [Hardware measurements and timings](capture.json)

## Reproduce the capture

Run from the repository root on the Mac with the capture GPU available. The directory named by `TSS_CORAL_CAPTURE_DIR` must be new. The bundled front cache used here has SHA-256 `f45365cc3fb8808fb172d535cf15e31f237f9a661a57bf100df3b2b8524394e1`.

```sh
CC=/usr/bin/clang CXX=/usr/bin/clang++ RUSTUP_TOOLCHAIN=1.88.0 \
TSS_LIVE_ADAPTER='Radeon Pro 5300M' TSS_CORAL_CAPTURE_DIR="$PWD/coral-wheel-capture" \
cargo test --locked --manifest-path live-wheel/Cargo.toml --lib \
  coral_capture::capture_coral_wheel -- --exact --ignored --test-threads=1
```
