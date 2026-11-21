# Two-wheel Windows performance capture

The [three-second recording](wheel-stack.mp4) shows all four films in one native projection. Violet / Blue is closest to the lamp at full opacity. Yellow / Red is in front at 70%. The Windows editor applies the mouse tug to the rear film of Violet / Blue while both wheels run. [Original optical frame 60](wheel-stack.png)

With the Violet / Blue source fixed at 0.65 seconds, advancing only Yellow / Red from 0.2 to 2.0 seconds changed 101,440 of 230,400 projected pixels by at least three RGB codes in one or more channels. Both films on each wheel were visible. The test used finite-pixel optics and each wheel’s default focus. [Configuration and frozen comparison](benchmark.json)

The GTX 1650 Ti Max-Q presented 481 frames at 640 × 360 on a 2571 × 1444 Windows surface during the separate 12-second timing trial. The measured rate was 39.9894 FPS, with 481 applied mouse movements and zero skipped acquisitions. Median, 95th- and 99th-percentile intervals were 24.9971, 25.2859 and 26.3448 ms. The 11 NVIDIA driver samples inside the timing window peaked at 38% GPU utilization, 156 MiB and 53 °C. [Presentation timings](benchmark.json) · [Device telemetry](gpu.csv)

The optical capture retained 121 successful presentations and their timestamps. The H.264 viewing copy contains 122 decoded frames across 3.016 seconds, including the final hold. Encoding used the editor’s sRGB-to-Rec.709 transform. [Source capture](capture.json) · [Recorded presentation intervals](frames.ffconcat)

The tested executable was built from source `2a2ace1`. Its SHA-256 is `5470f1c5bf85db4b1e6a86c6794055afec7050f222ad95fcaf0c795b8e175b60`. Follow the [guarded runner instructions](../../coral-wheel/windows/README.md) and add `-WheelStack` to the 640-pixel run. The editor selects both wheels through its ordinary layer controls.

## 4K native movie and projector

The [two-second wheel-stack movie](wheel-stack-4k.mp4) contains 60 decoded H.264 frames at 3840 × 2160 and 30 FPS. Its stereo AAC and video streams both start at zero and last two seconds. The first audio cue drives a rear-film tug and the second drives front-film rotation on Violet / Blue. Yellow / Red advances at 70% opacity above it. The two source-to-export audio correlations measured 0.9941 and greater than 0.99999. The [editor verification](wheel-stack-4k.webview.json) also records 19 projector presents and zero offscreen frames while both displays were minimized.

At frame 42, the decoded movie measured 55.87 dB average YUV PSNR against its own raw optical readback through the export color transform. Against the [previous same-cue, single-wheel raw frame](../../windows-live/gesture-controls/export/frame-0042.png), all 8,294,400 pixels in the two-wheel raw frame differed by at least three RGB codes in one or more channels. The mean absolute difference across all channels was 30.50 codes. The frozen-clock comparison above isolates the second wheel’s motion in the same executable.

To reproduce on the qualifying Windows test system, use the existing guarded export fixture with `--wheel-stack-trial --verify-wheel-export <absolute-audio-path> <new-mp4-path>` and set `TSS_WHEEL_EXPORT_PROOF` to a new PPM path and `TSS_WHEEL_EXPORT_PROOF_FRAME=42`.
