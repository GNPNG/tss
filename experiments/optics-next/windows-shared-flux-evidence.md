# Windows shared-flux 4K diagnostic

This is an experimental, silent, 8-bit diagnostic. Its pale, flat appearance does not match the supplied projector reference, and its optical model and Beer-law behavior are not validated by this render. The source is different from any subsequent Beer-law test revision.

Input consisted of 59 consecutive states from steps 2–60 of one 160 × 160, two-component Sailfish simulation. Two selected pigment channels were computed by CPU replay from the GPU solver's per-step net link transfers. The replay's discrete mass budget was checked; diffusion and phase confinement were not validated. The optical renderer constructs an approximate meniscus and does not model a measured three-dimensional projector or liquid volume. The input was little-endian float32 with layout `[59, 5, 160, 160]`: signed distance, `rho`, `phi`, dye-rho, dye-phi. SHA-256 `9607f73b6739469c081c9b8a85990b3faddb2483abd250d4b0a61add45360972`.

The source snapshot was the frozen timing revision of `tracer.wgsl` (`d838001ff0543c0b2092d57d153aca1ac08e6b4c82a0d6273ed4e3e6d76c3830`), `src/bin/tracer.rs` (`9e2f642155764871f9f0efc4d21c231a4021b6dd687b528d50e0e06d98cdf547`) and `tiles.rs` (`a71feb6dd70ce8c90e5fed8692085aa830167c916189b1dd24339c4cd64d0008`). Windows selected NVIDIA GeForce GTX 1650 Ti with Max-Q Design, Vulkan, driver 617.14. The executable was run with the process settings `TSS_TILE_ROWS=24`, `TSS_REPORT_TILE_TIMES=1` and the installed FFmpeg binary directory on PATH:

```bat
target\release\tracer.exe vulkan shared-flux-59.f32le 160 160 1 59 3840 2160 15 out-shared-flux-59 f38ed0 29d4dd 6078d9 ffffff 0 2 0 4 0 0.006 1.333 1.36 0.30
```

The Python child-process monitor terminated the child if a reported host tile wait exceeded 250 ms, if output stalled for 6 seconds between tiles, if startup or frame finalization stalled for 30 seconds, or if total runtime exceeded 120 seconds. It cannot prevent a GPU or driver hang before a measurement is emitted. The child exited 0 after 35.563 seconds. [The complete log](output/windows-shared-flux-4k-59frame-timings.log) contains 5,310 tile measurements, 3.386–19.818 ms, median 4.528 ms; these measure host submit-to-poll elapsed time, not GPU timestamp duration or real-time performance.

[The actual video](output/windows-shared-flux-4k-59frame-diagnostic.mp4) decoded to 59 frames of 3840 × 2160 H.264/yuv420p at 15 fps, with no all-black decoded pixels. The 15 fps is presentation timing and is not calibrated to physical time. All consecutive decoded frame pairs differed, but compression can contribute to pixel differences. Video SHA-256: `b22090d24e1d3ee485acc43dc30c59325d4dbb98556f36e0d613db3264b8b7a4`.

The lossless readbacks are [frame 0](output/windows-shared-flux-4k-frame-0000.png), [frame 29](output/windows-shared-flux-4k-frame-0029.png), and [frame 58](output/windows-shared-flux-4k-frame-0058.png). Each was verified as 3840 × 2160 RGBA with alpha 255 throughout and no all-zero RGB pixels. The renderer checks the alpha channel for every GPU frame before encoding; the H.264 video has no alpha channel. These artifacts are not a 16-bit master, a photorealistic validation, or an approved visual.
