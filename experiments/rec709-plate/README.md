# Rec.709 plate conversion experiment

`plate.rs` is an isolated, unused sample-conversion component. It is not connected to the application and does not implement PNG export. Its input contract is linear-light Rec.709-primary straight-alpha RGBA as finite `f32` samples in an explicitly specified row order. The conversion clips to the unit interval, applies the Rec.709 OETF to RGB only, quantizes once to 16-bit, and writes top-down, little-endian RGBA samples. It does not transform the source gamut, unassociate premultiplied alpha, apply tone mapping, or create metadata.

Run the component tests with the pinned toolchain:

```sh
rustup run 1.88.0 rustc --test --edition 2021 experiments/rec709-plate/plate.rs -o /tmp/tss-rec709-plate-tests
/tmp/tss-rec709-plate-tests
```

On the development Mac, the converter's complete 3840 × 2160 output for the independent `/private/tmp/tss-plate-work/fixture-4k.rgba32fle` fixture matched the previously verified 16-bit raw output byte for byte. That raw output was encoded to PNG by FFmpeg, decoded back, and compared byte for byte. FFprobe reported `rgba64be`, full range, `gbr`, BT.709 primaries and transfer. These are isolated fixture checks, not application, Windows, display, or print-pipeline validation.

Integration requires a renderer capable of producing linear-light, straight-alpha float plates, verified framebuffer readback on each target, a transfer strategy that preserves float precision, and an encoder that writes and validates 16-bit PNG color metadata. The intended optical transmission of separate liquid colors must also be represented in the plate compositing contract; ordinary alpha-over cannot be assumed to reproduce it. Capability failures must be reported rather than converting an 8-bit canvas into nominally 16-bit output. Performance, ICC embedding, P3, and target-application interpretation remain unverified.
