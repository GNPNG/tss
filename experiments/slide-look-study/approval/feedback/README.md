# Recursive video feedback and audience delay

[Moving comparison, 3 seconds](feedback-comparison.mp4) · [Frame at 1.083 seconds](feedback-still.png)

The left pane shows the native wheel through Violet / Blue, Blue / Red and Blue / Green. The right pane shows the same optical frames and wheel changes with a 125 ms feedback delay, 78% strength, 1.06× zoom and 3° rotation per return. Each pane is 320 × 180; the clip contains 72 frames at 24 fps. Both runs used the native Rust/wgpu renderer through a CPU Vulkan device. This is an effect comparison. The Windows live frame-rate measurements used a separate run.

## Native program signal

For cue time t the effect reads the newest retained *completed program* at or before t − delay. In linear RGB the new program is (1 − strength) × current + strength × transformed return. The return includes earlier feedback. Rays that fall outside the transformed gate sample the current frame. The native output encodes sRGB after composition. The operator can adjust delay from 25 to 450 ms, strength from 0% to 95%, zoom from 85% to 115% and rotation from −12° to +12°. Disabling the effect releases its textures.

Sixteen RGBA16F images hold the recursive history at up to 640 × 360, totaling 28.125 MiB at that resolution. Two full-resolution RGBA16F work images consume an additional 3.516 MiB at 640 × 360. Actual tap age includes frame and retention cadence. A discontinuous seek or backwards time starts a fresh history. Transport anchors carry song position; without audio the native performance clock supplies the timestamps.

The separate audience queue stores the completed display-referred program including any feedback. Its 0–1000 ms delay uses monotonic presentation deadlines and keeps the song timestamp of each image. Before the first image is due the output is black. Natural song end allows queued images to drain before the last image holds; a transport epoch change clears the queue. Storage is bounded at 43 RGBA8 images of up to 640 × 360, 37.8 MiB, with on-GPU scaling and letterboxing when presented to the audience. The same wgpu device and queue drive the editor and audience surfaces. The Windows host opens borderless on another monitor when available, or in a movable window on a single display. Escape or window close disconnects the audience window.

## Measurements

With a 100 ms delay and strength 0.6, the program sampled time 0.105 s at time 0.208 s; the earlier frame had itself sampled 0.000 s. Across 27,565 unclipped RGB channel values, mean absolute linear-light error was 0.00110 against the recursive prediction and 0.07131 against a single delayed-source mix. Two successive frames of one wheel had different source pixels. Rewind cleared the old program and disabling the effect restored the original source pixels.

In a separate 128 × 72 test with active feedback, two projector frames were black while priming. Three delayed frames reproduced every corresponding cue RGBA byte, including the final frame after natural end. Clearing the queue removed the previous program. This tests the rendered pictures and deadline selector. Physical scan-out and audio-device latency require timing at the actual outputs.

## Reproduction

Select a CPU-only software Vulkan ICD using `VK_DRIVER_FILES`. From the repository root run the following native tests.

```sh
cargo test --locked --manifest-path live-wheel/Cargo.toml --features wgpu/vulkan-portability --lib timestamped_feedback_reads_completed_program -- --ignored --nocapture
cargo test --locked --manifest-path live-wheel/Cargo.toml --features wgpu/vulkan-portability --lib projected_program_matches_delayed_images_and_drains -- --ignored --nocapture
cargo test --locked --manifest-path live-wheel/Cargo.toml --lib audience_deadlines_survive_transport_end
```

To regenerate the paired frames, create an empty directory, set `TSS_FEEDBACK_CLIP_DIR` to it and run the first command with test name `record_feedback_comparison`. The native test writes `source-000.ppm` and `feedback-000.ppm` through index 071. Render the comparison with FFmpeg.

```sh
ffmpeg -framerate 24 -i "$TSS_FEEDBACK_CLIP_DIR/source-%03d.ppm" \
  -framerate 24 -i "$TSS_FEEDBACK_CLIP_DIR/feedback-%03d.ppm" \
  -filter_complex '[0:v][1:v]hstack,format=yuv420p' -c:v libx264 -crf 16 \
  -movflags +faststart feedback-comparison.mp4
```
