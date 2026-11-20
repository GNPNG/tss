# Windows live-wheel benchmark and front-slide continuity

The native Windows MSVC release on the Windows test laptop used the GeForce GTX 1650 Ti Max-Q Vulkan adapter. The optics pass rendered at 640 × 360 and presented to a 2571 × 1444 child surface. The source held two 512 × 288 distance fields per time step. The lower-resolution optical pass used 24 pupil samples. The FIFO surface requested a maximum of two frames in flight. Each preset had a two-second warmup followed by twelve seconds of measurement. Mouse messages drove circular drags on the rear slide. The glass gap stayed fixed.

| Wheel | Successful presents | FPS | p95 interval (ms) |
|---|---:|---:|---:|
| Violet / Blue | 481 | 40.00 | 25.30 |
| Blue / Red | 482 | 40.00 | 25.25 |
| Yellow / Red | 481 | 40.00 | 25.42 |
| Blue / Green | 481 | 40.00 | 25.29 |
| Blue / Yellow | 480 | 39.98 | 25.44 |
| Violet / Green | 481 | 39.97 | 25.29 |
| Violet / Red | 480 | 39.92 | 25.36 |

The 3,366 successful presents had zero reported skipped acquires during the timed windows. Each interval ends when the host returns from FIFO present. The guard sampled the discrete GPU 99 times over acquisition and execution, observing maxima of 43% utilisation, 140 MiB allocated, 58 °C and 14.34 W. The reported FPS includes presentation pacing. The telemetry does not isolate GPU execution time or latency to scanout. [Baseline JSON](benchmark.json) · [Device telemetry](gpu-public.csv)

## What the startup demonstration showed

The benchmark visibly drew frozen-time verification frames at startup. It swirled the rear slide at t = 0.65 s, switched focus directly between both endpoints, then resumed normal playback. The frozen gesture changed 36,520 pixels by at least three 8-bit codes. The repeated untreated control changed zero. The two focus endpoints differed by that threshold at 55,172 of 230,400 pixels. These operations can produce a jump while the fluid state is stationary. The wheel picker also changes palette between timed intervals. [Frozen state at rear focus (left) and front focus (right)](frozen-focus.png)

## Front-slide continuity

The [source video](slide-footprints.mp4) shows the separate dyed regions exported by Basilisk before the optics pass. White is dye occupancy. Each panel downsamples its own 1024 × 576 phase-fraction frame to 512 × 288 for display. The [native analysis](field-continuity.cpp) uses the original PGM files and the exact packed cache, checking the dyed phase and clear carrier independently. An appearance or disappearance is a four-connected region with no footprint in the same phase of the adjacent frame. This includes small islands that travel their own width, so the counts give an upper bound on entire islands vanishing in place. A partial retreat of a large connected region does not meet this criterion.

Across the front slide's 96 original frames, the largest wholly appearing dyed region occupied 7 pixels, the largest wholly disappearing dyed region 6 pixels. In the live cache these maxima were 1 texel each. Clear carrier regions of up to 2 texels also appeared or disappeared. Repeating the comparison at 40 Hz with linear interpolation and the real forward/reverse timing gave the same maxima. For each adjacent pair the largest **net** change in front dyed area was 43 texels at 24 Hz, 75 at 40 Hz. These counts precede lens blur, optical overlap, display resampling and manual deformation. [Complete results](field-continuity.csv)

```sh
c++ -std=c++17 -O2 field-continuity.cpp -lz -o field-continuity
./field-continuity ../../../../live-wheel/assets/slide-fields.rg8.gz /path/to/solver-out
```

The independent [graded Windows 4K clip](../violet-cyan-4k.mp4) was also decoded to 640 × 360. A three-frame test flagged pixels where the first and last RGB values agreed within 4 codes in all channels and the middle pixel departed by at least 16 codes from both in the same channel and direction. Over 94 interior frames the largest connected flagged area was 3 pixels. At 32 and 64 codes it was 1 pixel. This optical control uses the encoded offline trajectory. The test detects short returning excursions, including some physically moving edges. It does not detect a step that lasts several frames or a compositor/scanout artifact. [Analyzer](temporal-contrast.cpp)

## Offscreen verification and retest

The benchmark runs startup verification through the same optical pipeline on a temporary track, keeps the verification images offscreen and discards that track before timed playback. The repeated untreated image stayed byte-identical and the rear-gesture and focus changes reproduced exactly. The source includes an offscreen/visible-path parity test. [Host proof](../../../../src-tauri/src/live/windows/benchmark.rs) · [Renderer](../../../../live-wheel/src/lib.rs)

A second physical run completed 3,336 successful presents with zero reported acquire skips. Blue / Red averaged 37.70 FPS (p95 25.65 ms, p99 35.99 ms). The remaining six averaged 39.97–40.00 FPS. The guard's 95 samples peaked at 40% GPU utilisation, 142 MiB, 55 °C and 14.11 W. Attributing the slower Blue / Red interval to the shader would require isolating its GPU execution time. [Retest JSON](benchmark-after-isolation.json) · [Retest telemetry](gpu-after-isolation-public.csv)

## Steady-motion capture on the Windows GPU

The ordinary editor recorded each successful present for 8.007 seconds with Violet / Blue selected, the original optical focus profile held constant and zero mouse events. The 322 optical target readbacks cover both changes in direction of the 96-state trajectory. The viewing encode uses the recorded presentation intervals and adds a final hold. Analysis used the original RGB samples. [Windows recording](violet-blue-live.mp4) · [Capture configuration](motion-capture.json) · [Timestamps](motion-timestamps.ffconcat)

At captured frame 121 (2.982 s into the recording, 3.95754 s of source time), 142 pixels met the 16-code return criterion. The largest connected region was 13 pixels. At frame 279 near the other reversal 13 pixels met the criterion. Away from both turnarounds the maximum was 5 pixels. The 3-frame-radius comparison, spanning approximately 150 ms between the outside frames, flagged 2,681 and 2,242 pixels at the respective turns. [Immediate neighbors](steady-excursions-r1.csv) · [Neighbors three frames away](steady-excursions-r3.csv)

### Preview timing change

The original preview reflected the source-time coordinate at each end of the trajectory. Position was continuous, but interface velocities immediately reversed sign. The live preview now uses a cubic Hermite transition over eight source intervals at each end. For endpoint distance d and transition width w, the remapped distance is `d × (d/w) × (2 − d/w)`. The endpoint velocity is zero and the transition rejoins the interior with unit slope. Both end states and the original 190/24 s round-trip duration are preserved. The peak speed inside the transition is 4/3 of the source rate. The offline solver trajectory, optical coefficients and gestures are independent of this replay control. [Playback implementation](../../../../live-wheel/src/fields.rs)

SwiftShader ran the native optical pass on the same 640 × 360 grid at the 13 Windows timestamps surrounding each turn. All settings, including Violet / Blue focus, were held constant across the two revisions. At the exact reported frames the one-frame 16-code excursions went from 141 and 13 to zero at each turn. Across the full 13-frame neighborhoods the eased maxima were 0 and 7. This measures the specific return signature and does not identify every artifact a viewer could see under mouse input or in another wheel.

| At Windows capture frame | Original GPU pixels (±1 frame) | Original software replay | Eased software replay | Original software (±3 frames) | Eased software (±3 frames) |
|---|---:|---:|---:|---:|---:|
| 121 | 142 | 141 | 0 | 2,760 | 575 |
| 279 | 13 | 13 | 0 | 2,263 | 254 |

To repeat the optical check, build the Rust software Vulkan test with an isolated CPU Vulkan ICD. Set `TSS_SOFTWARE_REPLAY_TIMES` to a file with one absolute source time in seconds per line and `TSS_SOFTWARE_REPLAY_OUTPUT` to a new empty directory. The test writes one lossless P6 optical capture per supplied time. For fixed-time samples, the actual field sample time is the captured source start plus the cumulative preceding durations from the FFconcat manifest. Use frames 115 through 127 and 273 through 285 as two separate runs of the temporal analyzer. Source occupancy diagnostics above describe the original replay clock.

Public telemetry uses a device ordinal within each run. All sampled times, device indices and numeric measurements retain their recorded values.
