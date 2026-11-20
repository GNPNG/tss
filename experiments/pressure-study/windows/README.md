# Windows pressure trial

The Windows x64 preview ran on a GeForce GTX 1650 Ti with Max-Q Design and 4 GiB of VRAM. The instrumented editor sent mouse events through its normal native window. The 640 × 360 optical pass and the 4K export ran sequentially under the GPU monitor. [Live footage](violet-blue-pressure.mp4) · [Two-second 4K export with audio](two-slide-pressure-4k.mp4) · [4K frame 42](frame-0042.png)

| Violet / Blue rear slide | Measured |
|---|---:|
| Presentation rate | 40.00037 FPS |
| Median / p95 / p99 frame interval | 25.006 / 25.355 / 25.626 ms |
| Presented / skipped | 481 / 0 |
| Applied mouse movements | 283 |
| Maximum sampled GPU utilization / memory / temperature | 35% / 148 MiB / 53 °C |

The frame-rate measurement covers a 12-second run. Separate readbacks produce the short live movie. GPU telemetry is sampled, and frame intervals measure successful presentation on the host. [Benchmark and pressure traces](benchmark.json) · [21 live telemetry samples](live-gpu.csv)

## Frozen-slide checks

At 50% mouse pressure, the rear and front slides each changed independently. The table counts pixels with at least three RGB code values of difference in any channel. Each stationary hold was byte-identical across repeated renders. Removing the load restored the neutral image. Replaying the captured pressure reproduced both the peak and release.

| Slide and contact, normalized XY | Changed pixels |
|---|---:|
| Rear, (0.48, 0.52) | 11,198 |
| Front, (0.48, 0.52) | 17,301 |
| Front, (0.58, 0.41) | 21,334 |

An initial trial failed a combined front-slide assertion without recording the individual differences. The released test retains that coordinate, checks a second front contact and waits for the actual pressure plateau. At the second contact, a 136 ms wall-clock wait sampled 0.494385 of the requested 0.500000 load. That intermediate frame differed from the settled frame in 5,229 channel bytes. The final trial passed all three points. The original failure did not preserve enough information to identify which comparison failed. [Front at the original point](frozen-front-original.png) · [Second front point](frozen-front.png) · [Rear point](frozen-rear.png)

## Export and performer inputs

The native wheel exported 60 frames at 3840 × 2160 and 30 FPS with Rec.709 H.264 video and two seconds of stereo 48 kHz AAC. Scheduled rotation and front compression used the existing two-tone audio fixture. The compressed frame at 1.4 s can be compared to the native readback. [Stream inspection](streams.json) · [Frame 42 comparison](frame-0042-psnr.txt) · [Export telemetry](export-gpu.csv) · [Source M4A](../../slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a)

Two separately paired loopback HTTPS clients exercised the packaged Go listener, WebView2 controls and native recorder. The rear pressure channel reported 0 → 0.75 → 0 while the front received a Pencil down, move and up. The projector opened and closed on the single attached display. Physical Force Touch, Pencil, Wi-Fi and a second physical display remain hardware checks. [Editor report](webview.json)

The release executable SHA-256 is `745b3178fe4ebfd6e2e48767bacdbb4434c83e70e46cff1ac9624d0407248bfd`. The performance-surface executable SHA-256 is `79ba0e928c13df5be07962167c4a1d1b825a7d6e752a41e2cea24d0a7f1eb982`. Both executables are unsigned. [Windows release](https://github.com/GNPNG/tss/releases/tag/v0.1.0-preview.2)

The live capture contains 122 GPU readbacks spanning 3.008945 s. Its viewing copy uses their presentation timestamps with a final endpoint frame, yielding 123 encoded frames and 3.010 s. The prerelease asset carries this timing correction. [Live viewing copy](https://github.com/GNPNG/tss/releases/download/v0.1.0-preview.2/violet-blue-pressure.mp4)
