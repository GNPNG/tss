# Windows gesture recordings and frame timings

The native Windows editor presented the wheel in its own child window on the GTX 1650 Ti Max-Q through Vulkan. The optical target was 640 × 360, the child surface 2571 × 1444, and the FIFO queue limit two frames. The 704 × 704 two-slide cache has a 512 × 288 centered gate. Each run used track 1, wheel focus, and Medium material response. Scripted Windows button and move messages traversed the same child-window handler used for an operator's mouse.

## Corrected optical pass

| Violet / Blue, rear slide | FPS | Median interval | p95 | p99 | Presents | Skips |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Material tug | 40.00 | 25.00 ms | 25.24 ms | 25.40 ms | 481 | 0 |
| Rotate slide | 39.97 | 25.00 ms | 25.45 ms | 26.34 ms | 480 | 0 |

Each mode ran through a three-second recording, followed by two seconds of warm-up and a twelve-second measurement with readback disabled. Intervals were measured after consecutive successful FIFO present returns. The two windows include no skipped surface acquisitions. GPU telemetry across the whole invocation peaked at 36% utilization, 150 MiB allocated, 59 °C and 15.85 W. [Timings](finite-pixel.json) · [Telemetry](finite-pixel-gpu-public.csv)

The recordings show the native optical target captured next to presentation. The rotation recording follows the tug on the same performance track. Both runs retain the natural field motion. The viewing encodes follow the recorded frame timestamps. [Material tug video](violet-blue-tug.mp4) · [Rotate slide video](violet-blue-rotate.mp4) · [Tug capture](tug-capture.json) · [Rotation capture](rotate-capture.json)

The frozen-time control applied the same 36-move arc independently to each mode on a fresh track at t = 0.65 s. The rear tug changed 11,518 of 230,400 pixels by at least three codes in one or more RGB channels. Rear rotation changed 169,245. Native replay consumed each 38-point recording in batches of eight and reproduced every byte of its optical image. Duplicate untreated frames were also identical. This control measures final-pose equivalence between native recording and replay. Each timed window applied real-time mouse events continuously. [Untreated image](frozen-before.png) · [Tug](frozen-tug.png) · [Rotation](frozen-rotate.png)

## Seven-wheel reference comparison

| Wheel | Slide | Action | FPS | p95 |
| --- | --- | --- | ---: | ---: |
| Violet / Blue | Rear | Tug | 39.95 | 25.68 ms |
| Violet / Blue | Rear | Rotate | 40.00 | 25.22 ms |
| Blue / Red | Front | Tug | 40.00 | 25.18 ms |
| Yellow / Red | Front | Rotate | 39.98 | 25.44 ms |
| Blue / Green | Rear | Tug | 39.92 | 25.53 ms |
| Blue / Yellow | Rear | Rotate | 39.99 | 25.26 ms |
| Violet / Green | Front | Tug | 39.97 | 25.55 ms |
| Violet / Red | Front | Rotate | 39.99 | 25.48 ms |

This invocation used the reference optical sampler throughout, with the same wheel geometry, pose model and presentation path. The editor reported 3,844 successful measured presents across the eight windows and zero skipped acquisitions. Peak telemetry was 40% utilization, 150 MiB allocated, 59 °C and 14.35 W. [Full timings](reference.json) · [Telemetry](reference-gpu-public.csv)

The executable for these two runs has SHA-256 `3a3bc7b4547f4f56a723bd7a7ed74955fe6d92762300059e272c5236bcd9b0e1`. Use `--benchmark-live <new-report.json>` for the eight-window schedule and append `--gesture-pair` for the two Violet / Blue windows. Set `TSS_LIVE_FOOTPRINT=0` for the reference and `TSS_LIVE_FOOTPRINT=1` for the corrected optical pass. The destination directory must exist. The benchmark also creates its PPM proofs and two timestamped frame sequences beside the report.

Public telemetry uses a device ordinal within each run. All sampled times, device indices and numeric measurements retain their recorded values.
