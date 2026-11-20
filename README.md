# The Sacred Swirls

![The Sacred Swirls project logo](WebAssets/assets/tss-project-logo.jpeg)

The Sacred Swirls (TSS), EPNI-018, is a desktop liquid-light instrument. Two independently simulated films sit between parallel glass plates. Each has its own dye, carrier, in-plane motion and optical depth. The Windows editor projects light through both films with a shared adjustable focus.

## Current development build

[Windows download](https://github.com/GNPNG/tss/releases/tag/v0.1.0-preview.1) · [Start performing](docs/windows-preview.md)

- Open MP3 or M4A audio and preview a waveform with synchronized liquid-light motion.
- Add, select, hide, reorder and remove liquid layers. Adjust each layer's palette, hue, flow speed, scale and opacity.
- Export an 8-bit development preview of the current liquid composition and audio as 3840×2160 H.264/AAC MP4 at 30 fps. Rendering runs frame by frame and can be cancelled.
- On Windows, export the selected Mathmos wheel and scheduled slide gestures in 4K on the same GPU used for live preview. [Starting poses and native MP4 output](live-wheel/README.md#export-the-selected-wheel)

Windows opens on the Violet / Blue wheel. Choose a wheel, **Rear slide** or **Front slide**, then **Material tug** or **Rotate slide** and drag on the image. The glass extends beyond the visible gate. Move **Projector focus** to bring either film into focus. [Live controls](live-wheel/README.md) · [Seven wheel studies](experiments/slide-look-study/approval/wheels/README.md) · [Focus pull](experiments/slide-look-study/approval/focus/violet-blue-rack-focus.mp4)

Open a song, expand **Gesture macros** and select **Record gesture**. You can replay a take at its original song position or attach it to a recurring musical or lyric phrase. Each macro has an enable switch, per-occurrence mutes, amount, gain per recurrence and a ceiling. [Recording and trigger controls](experiments/phrase-triggers/README.md)

The native wheel supports recursive video feedback and a separate audience window with adjustable A/V delay. The selected-wheel exporter renders 3840 × 2160 H.264/AAC from the native optics, song and scheduled gestures. [Projector and cue audio](experiments/projector-output/README.md) · [Feedback comparison](experiments/slide-look-study/approval/feedback/feedback-comparison.mp4)

The GTX 1650 Ti Max-Q delivered 39.95–40.00 FPS across all seven wheels at 640 × 360 with finite-pixel optical sampling. Both Violet / Blue gesture tests ran at 40.00 FPS. All 3,843 measured presentations succeeded. The frozen-frame capture/replay test reproduced both 38-event gestures byte for byte. [Windows measurements and recordings](experiments/slide-look-study/approval/windows-live/gesture-controls/README.md)

## Experiments

The [library and demo catalog](docs/library-catalog.md) records the implementation language, GPU API, model, potential TSS use, port decision and test evidence for each candidate. [Sortable CSV](docs/library-catalog.csv) · [Second-pass material comparisons](https://gitea.gnomatix.com/gnpng/org/issues/13)

These independent studies cover the input, controller and production pipelines.

- [Gesture input](experiments/gesture-input/README.md) captures WebKit Force Touch and pointer events for later layer control. Physical Force Touch devices and Apple Pencil have not been verified.
- [Wireless control](experiments/wireless-control/README.md) tests authenticated, separately identified controller sessions. It does not yet control a visual or implement an iPad application.
- [Liquid fluid](experiments/liquid-fluid/README.md) studies separate evolving color fields and gesture forces. Its appearance is not yet an acceptable match to the references.
- [Rec.709 plate conversion](experiments/rec709-plate/README.md) tests conversion of genuine floating-point samples to 16-bit samples. It does not yet produce application exports.

The intended performance controls include Apple Pencil swirl, trackpad pressure for compression, and configurable assignments between input devices, layers and controls. Concurrent physical-device behavior and deterministic recording remain to be established. See the [liquid-light research](docs/liquid-light-research.md) for the rendering target and current limitations.

## Run from source

Install Rust 1.88 and the platform prerequisites for Tauri 2, plus FFmpeg with the `libx264` and AAC encoders available on `PATH`. The pinned Rust toolchain is declared in `rust-toolchain.toml`.

```sh
mise run dev
```

Or run `cargo run --manifest-path src-tauri/Cargo.toml`. The first build downloads Rust dependencies. A release build is available with `mise run build`.

On a macOS system where a third-party Clang installation is unavailable, select Apple's compiler for a build.

```sh
CC=/usr/bin/clang CXX=/usr/bin/clang++ mise run dev
```

Audio files are read locally. Export writes a new MP4 and refuses to overwrite an existing file. Rendering and encoding speed depends on the device and composition.

## Verification

`mise run test` runs the Rust tests. The repository also tracks ongoing work in the GNPNG TSS epic and its linked issues.
