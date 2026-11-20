# Windows performance with the corrected optics

The final optical path applies the same finite-pixel integration at rest and throughout a gesture. On the 640 × 360 preview, the same localized rear pull now changes 2,673 pixels at the three-code threshold. A quarter turn of the rear glass changes 170,006. Each gesture's 38 recorded points reproduced its frozen image byte for byte after playback through the native gesture state machine. [Before](frozen-before.png) · [After material tug](frozen-tug.png) · [After rigid rotation](frozen-rotate.png)

| Wheel | Slide | Action | FPS | p95 frame interval |
| --- | --- | --- | ---: | ---: |
| Violet / Blue | Rear | Tug | 40.00 | 25.33 ms |
| Violet / Blue | Rear | Rotate | 40.00 | 25.32 ms |
| Blue / Red | Front | Tug | 39.95 | 25.40 ms |
| Yellow / Red | Front | Rotate | 39.99 | 25.20 ms |
| Blue / Green | Rear | Tug | 40.00 | 25.20 ms |
| Blue / Yellow | Rear | Rotate | 40.00 | 25.24 ms |
| Violet / Green | Front | Tug | 39.98 | 25.51 ms |
| Violet / Red | Front | Rotate | 39.99 | 25.19 ms |

The native editor returned from 3,843 successful FIFO presents across these timed windows. Surface acquisition skipped zero frames. Peak readings over 114 guard samples were 37% GPU utilization, 150 MiB allocated, 56 °C and 15.77 W. Actual presentation went to a 2571 × 1444 child window on the NVIDIA GTX 1650 Ti Max-Q Vulkan adapter. Each measured interval followed two seconds of warm-up. The preceding three-second Violet / Blue recordings retain the native frames and their actual timing separately from these twelve-second performance samples. [Full timings](benchmark.json) · [GPU telemetry](gpu-public.csv) · [Tug video](violet-blue-tug.mp4) · [Rotate slide video](violet-blue-rotate.mp4) · [4K export](../export/two-slide-cues.mp4)

Public telemetry uses a device ordinal within each run. All sampled times, device indices and numeric measurements retain their recorded values.
