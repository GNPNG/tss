# Stereo pressure on three live oil slides

Play [the five-second H.264 clip](oil-pressure-half-sbs.mp4) on a stereoscopic TV in **side-by-side, left eye first** mode. The MP4 contains 120 frames at 24 fps, 1920 × 1080, BT.709 limited-range 4:2:0. Each eye was rendered at 960 × 540. The encoder places the eyes side by side, doubles the height and marks the frame packing as left/right. The TV expands each half to the display width. The [left-eye still](left-eye-preview.png) shows the press at its peak.

Three native Basilisk workers evolve three sealed oil cells. The native projector renders one view per eye before compositing the cells’ transmission. Both eyes read the same simulation time at every layer and frame, verified in the capture. A 24 fps performance clock sets the simulation cadence independently of the time taken to render and encode the clip. The stereo path is exercised by an offline GPU study.

## Pressure and stereo geometry

A performer presses only the front slide at gate coordinates (0.6245, 0.3565). The capture finds this contact from a horizontal color gradient on the actual oil boundary. Pressure ramps from frame 10 to 36, holds through frame 70, and releases by frame 96. A smooth cubic envelope drives the editor's volume-preserving XY deformation and optical thickness. At maximum load the local gap is 35% of its nominal 1 mm. The source-side glass stays fixed, so the oil midpoint at the contact recedes by 0.325 mm. The rear and middle cells retain their original geometry.

The rectified eye views converge at the middle slide, 52.6 mm from the modeled optics. The rear and front planes lie at 53.4 and 51.8 mm. For depth z in mm, horizontal disparity (right minus left, in eye pixels) is −2 W k (1/z − 1/52.6), with square-pixel eye width W = 960 and k = 14 mm. The product k of normalized focal length and half-baseline is an artistic stereo gain. It does not specify a measured vintage binocular projector. In the pressed region the shader evaluates z at the local oil midpoint and inverts the horizontal projection with three iterations. Each eye samples the actual oil geometry through that eye's coordinates; lamp illumination and incoming light retain their screen registration. A mono projection uses zero eye scale. This is thin-layer parallax with local relief from the gap model. It does not trace multiple scattering or solve pressure-driven film drainage.

| Layer | Neutral R−L, eye px | Full pressure R−L, eye px |
|---|---:|---:|
| Rear | +8 | +8 |
| Middle (convergence) | 0 | 0 |
| Front at the contact | −8 | −5 |

The [disparity measurements](disparity.csv) use a symmetric horizontal RGB patch match on individual films held at the same tick. The rear left-eye image was byte-identical before and after pressing the front. Left and right images of the middle film were byte-identical. The [performance trace](performance.csv) records frame time, pressure, local gap and all three presented fluid ticks. Every pair agrees on ticks 0 through 119. The video contains no audio.

On the Radeon Pro 5300M, the original independent-film focus test was repeated with zero eye scale. All 96 raw mono frames (88,473,600 RGBA bytes), the six stills and the focus CSV matched the pre-stereo capture byte for byte.

On 2026-11-28, the Intel Mac played this clip full-screen over HDMI to a Samsung 3D plasma at 1920 × 1080, 60 Hz. During the press sequence, the viewer confirmed one fused image with visible depth through the glasses. This checks stereoscopic playback on that installation. The press was recorded in the capture.

## Reproduce the capture

Build the native live solver as described in the [live solver guide](../live-slide-solver/README.md). Use Rust 1.88, the project's CImg and Basilisk paths, the native shader compiler, FFmpeg with libx264, and one available discrete GPU. From the repository root, set `TSS_LIVE_SOLVER` to the executable, `TSS_LIVE_ADAPTER` to a matching adapter name, and `TSS_LIVE_CAPTURE` to a fresh output directory with an existing parent.

```sh
cargo test --locked --manifest-path live-wheel/Cargo.toml --features live-solver \
  --lib stereo_capture::render_stereo_pressure_performance -- --ignored --exact --nocapture
```

The study writes `oil-pressure-half-sbs.mp4`, `disparity.csv` and `performance.csv`. To extract the full packed still or a conventional single-eye preview:

```sh
ffmpeg -n -ss 2.2 -i "$TSS_LIVE_CAPTURE/oil-pressure-half-sbs.mp4" -frames:v 1 full-pressure-sbs.png
ffmpeg -n -i full-pressure-sbs.png -vf 'crop=iw/2:ih:0:0,scale=640:360' -frames:v 1 left-eye-preview.png
```
