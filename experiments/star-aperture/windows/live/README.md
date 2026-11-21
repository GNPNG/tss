# Live keyed footage and moving matte

The [three-second live recording](live-media.mp4) shows a 960 × 540 native editor preview at its original presentation times. The green-screen star, hidden travelling matte and Violet / Blue wheel all advance on the editor performance clock. A scripted left-button drag tugs the rear film throughout. The capture retained every successful present before its endpoint and precedes the timed interval. [Native capture record](capture.json) · [Frame durations](frames.ffconcat)

The Windows build from `224df17`, SHA-256 `4f04fdc146d4573661ccb932cfc34d050d402fdea46eb9b0a1a73662d2e3ab6b`, ran each trial under a separate GPU guard on the GTX 1650 Ti Max-Q. Each trial used two seconds of warm-up and twelve seconds of FIFO presentation timing. The final 960 × 540 run started after CPU package preparation had finished. The screen surface was 2571 × 1444, with finite-pixel optics enabled.

| Optical resolution and run | FPS | p50 (ms) | p95 (ms) | p99 (ms) | Presented / skipped | Changes per media source | Source loops |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| [640 × 360](benchmark-640.json) | 39.9983 | 24.9911 | 25.8690 | 26.1000 | 481 / 0 | 288 | 4 |
| [960 × 540, initial](benchmark-960.json) | 40.0002 | 24.9988 | 25.2027 | 25.7018 | 481 / 0 | 288 | 4 |
| [960 × 540, idle repeat](benchmark-960-idle.json) | 39.9766 | 25.0036 | 25.2448 | 26.0828 | 481 / 0 | 289 | 4 |

Each run observed all 72 distinct decoded packet timestamps for both the star and matte on frames that the surface actually presented. Each applied 481 mouse moves during the measured interval. Freezing wheel and effect time at 0.65 s, then advancing both sources from 0.2 to 1.8 s, changed 73,648 projected pixels at 640 × 360 and 160,134 at 960 × 540 by at least three RGB codes. Seeking back reproduced the first image byte for byte.

During the idle 960 × 540 timing interval, eleven telemetry samples peaked at 39% GPU use and 190 MiB. The 640 × 360 timing interval peaked at 33% and 174 MiB. [640 telemetry](gpu-640.csv) · [Initial 960 telemetry](gpu-960.csv) · [Idle repeat telemetry](gpu-960-idle.csv)

Set the same two fixture paths as in the [4K comparison](../README.md). Launch the guarded editor with `--benchmark-live <new-json-path> --media-trial` for 640 × 360. Add `--preview-960` for 960 × 540.
