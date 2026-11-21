# Visual media sources

Use a movie for full-colour or keyed footage, a transparent PNG for a static overlay, or an animated GIF for a repeating figure. A portable TSS project records the filename and the original file's SHA-256. Keep assets alongside the project when transferring it to another machine. Relinking checks the entire file against its fingerprint.

The native importer uses the local FFmpeg and FFprobe executables. Both tools must be visible to the desktop application. On Windows, the [editor setup](windows-preview.md) lists the FFmpeg package. On Intel or Apple Silicon macOS the importer checks the Homebrew bin directories and the application search path.

## Put footage over moving oil in the Windows source build

1. Select a Mathmos wheel and start its motion. For the small front droplets, choose **Violet / Blue → Wheel with coral beads**.
2. Choose **Add artwork** and open the file. The new track appears above the wheel. Use the arrows in the track list to move it behind a film stack.
3. Open **Artwork settings**. Choose **Original color and alpha** for a cutout PNG, GIF or full-colour cartoon. For green-screen footage, turn on **Apply chroma key**, sample the key colour with the colour control, and adjust **Key range** and **Key softness**. Choose **Cycling gradient** to recolour the surviving figure.
4. To cut a moving aperture, add its source as another artwork track. Select it as **Mask A** on the figure. Hide the mask's own track to keep only its effect on the figure. Add **Mask B** for intersection, union, subtraction or exclusive-or.
5. Open the song and choose **Export artwork program in 4K** for the timed projection and song audio. The projector shows the same native program during playback.

## Preparation and playback

Import prepares an independently seekable lossless PNG sample for each decoded frame in an app-cache MOV and indexes its presentation timestamp and duration. Media larger than 960 pixels on its longest edge gets a second, aspect-preserving preview. Both versions follow the source timestamps. Variable-frame-rate footage, GIF holds and reverse scrubbing use the frame covering the requested time. Full-resolution frames retain 8-bit straight alpha. Preview resizing filters premultiplied colour and alpha at 16-bit precision before returning to straight alpha.

The Rust reader decodes individual samples, caches up to 192 MiB of decoded frames and prefetches the following sample. FFmpeg runs during preparation. Cache files persist between sessions and can occupy substantially more space than a delivery codec. The first import takes longer than opening an already prepared source.

## Source formats

The picker accepts MP4, M4V, MOV, WebM, MKV, AVI, GIF, APNG, PNG, JPEG, WebP, TIFF and BMP. Actual decoding depends on the installed FFmpeg build. The decoded image limit is 33,554,432 pixels. For files with audio, only the first video stream is prepared. The project's song supplies program audio.

For predictable colour use an SDR sRGB source or Rec.709-tagged video. The reader converts Rec.709 transfer values to sRGB for the native compositor. It assumes untagged stills use sRGB and untagged YUV video uses Rec.709. Convert wide-gamut/HDR sources first. This media path prepares 8-bit overlays. The production still exporter has its own 16-bit output path.

The fixture tests import a transparent PNG, an animated GIF and a variable-frame-rate MOV, seek across a frame boundary in both directions, and check the colour at partially covered preview edges.

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml media::
```
