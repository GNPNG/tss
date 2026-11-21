# Windows presentation of Wheel with coral beads

The Violet / Blue wheel ran at 40.00 FPS at both optical resolutions on the GeForce GTX 1650 Ti with Max-Q Design. Across the two 12-second windows, the editor presented 962 frames and skipped zero surface acquisitions. The native Vulkan child window measured 2571 × 1444. The projected distance field was 512 × 288 and the optics used finite-pixel sampling.

| Optical resolution | FPS | Presents / skips | Median / p95 / p99 interval (ms) | Applied front-slide mouse moves |
|---|---:|---:|---:|---:|
| 640 × 360 | 39.99970 | 481 / 0 | 25.0044 / 25.2183 / 25.4970 | 475 |
| 960 × 540 | 40.00021 | 481 / 0 | 24.9914 / 25.2581 / 25.5466 | 475 |

At 90% front focus, each trial requested **Replay front beads** at approximately 0, 4 and 8 seconds. All six takes reached a successful presentation. Each replay received one window-local front-slide material tug with 158 applied movements. Request-to-first-present times were 49.92, 49.92 and 49.93 ms at 640 × 360 and 49.99, 50.41 and 50.05 ms at 960 × 540. Frame intervals use the host clock immediately after successful FIFO presentation returns. Two seconds of warm-up preceded each timed window. [640 report](width-640-benchmark.json) · [960 report](width-960-benchmark.json)

Before timing, the editor held the rear phase at 2 s and captured the front at 0, 1.9, 4 and 6 s. The front held byte-identically between 4 and 6 s and replayed byte-identically to its initial frame at both resolutions. Holding the front at 6 s and advancing the rear to 3.5 s changed 62,255 pixels at 640 × 360 and 139,997 at 960 × 540 by at least three RGB codes in one or more channels.

| Optical resolution | Timed / total GPU samples | Peak utilization | Peak VRAM | Peak temperature | Peak power |
|---|---:|---:|---:|---:|---:|
| 640 × 360 | 11 / 18 | 33% | 156 MiB | 52 °C | 14.12 W |
| 960 × 540 | 11 / 17 | 34% | 156 MiB | 54 °C | 15.00 W |

Each peak is the highest reading within the timed window. The guard sampled the NVIDIA driver at approximately one-second intervals. The CSVs retain the raw timestamps and measured values and identify the device by its run-local ordinal. Windows 11 Pro for Workstations Insider Preview build 28120 used NVIDIA driver 617.14. [640 telemetry](width-640-gpu.csv) · [960 telemetry](width-960-gpu.csv)

## Projected films in the Windows editor

The [8-second viewing copy](projected-films.mp4) follows the real **Slide motion** and **Projected films** controls through the ordinary Windows editor. At 0.95 s Windows UI Automation selects **Wheel with coral beads**. Both films are visible until 1.83 s, followed by **Front only**, **Rear only** at 2.63 s, **Clear glass** at 3.42 s and **Both films** again at 4.21 s. The caption under the projection follows the first native frame recorded for each configuration. Each UI Automation selection was read back from the editor control and independently recorded by the native renderer on a successful presentation. [Control readbacks](films-ui.json) · [Native frame and configuration manifest](films-capture.json)

The assembled editor was built from `36eb767`, with SHA-256 `0aa5ce8fc590214a6f62f0f075aea9dbc41b95a55e7411f8cf311f79d4a4af41`. Live readback captured 312 successful 640 × 360 projections spanning 8.013851 seconds on the GTX. Reading each frame back affects frame rate. The H.264 viewing copy retains the capture timing, with the repeated endpoint frame from the native presentation timing file. It runs 8.015 seconds, uses 8-bit limited-range Rec.709 and adds a 40-pixel caption band below the original image.

## Repeat the measurements

Build the native Windows editor from commit `ad58c6a` using the pinned Rust 1.88.0 toolchain and `cargo build --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol`. The timed executable SHA-256 was `25ac6fa36ea4f39b9363f169d994d75b376b7e957441f6927399f8bdb5455c98`. The earlier diagnostic executable rejected its 640-pixel attempt with “Insufficient interactive frame samples.” The successful version starts a fresh drag after each replay and checks the consumed points per take.

Use an interactive Windows desktop with the existing Rust GPU guard installed in the study workspace and both reservation paths clear. Set `$exe`, `$workspace` and the expected adapter UUID `$uuid` to the validated local values. Create separate empty output directories for the two runs. The runner requests exclusive cooperative use, NVIDIA idle preflight, a 120-second timeout and telemetry. It also checks for an active editor. [Guarded runner](run-beads.ps1) · [Guard protocol](../../../../gpu-job-control-rust/README.md)

```powershell
foreach ($width in 640,960) {
    $dir = (New-Item -ItemType Directory -Path "width-$width").FullName
    & .\run-beads.ps1 -Executable $exe -Workspace $workspace -RunDirectory $dir -ExpectedNvidiaUuid $uuid -PreviewWidth $width
    if ($LASTEXITCODE -ne 0 -or !(Test-Path "$dir\benchmark.json")) { throw "Trial failed at $width" }
}
```
