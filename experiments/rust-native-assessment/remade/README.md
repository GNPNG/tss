# Remade FFmpeg media compatibility

The candidate is the published [rff-cli 0.4.6](https://docs.rs/crate/rff-cli/0.4.6), upstream commit `482d511adfc5b323370367eb7f9e04b986c096ec`. This experiment passes explicit executable paths. TSS continues to use its installed FFmpeg.


## Observed result

The pinned CLI build's Rust H.264 encoder produced a 64,086-byte Annex B stream from all 60 moving coral frames at 160 × 90, preset Fast, QP 22. FFmpeg 9.0.1 independently decoded all 60 frames. Average YUV PSNR was 41.747067 dB, with a worst frame of 39.394234 dB, relative to the uncompressed 160 × 90 input. The input came from scaling the RC movie in FFmpeg. This comparison covers a CPU elementary encoder, downstream of pixel conversion. [Recorded hashes and measurements](result.json)

The full two-job CLI build was stopped before linking when available disk space dropped below 1.5 GiB. Cargo had compiled the two H.264 libraries used by the test. Its AV1 components invoked NASM during this build. Complete CLI exports remain untested. At the pinned revision, the candidate version banner and rffprobe program-version interface differ from the TSS media-tool detector. Its published filter list omits the production colorspace filter and upstream describes a software encoding path. [Upstream compatibility](https://github.com/Remade-With-Rust/remade_ffmpeg_rs/blob/482d511adfc5b323370367eb7f9e04b986c096ec/docs/compatibility.md) · [TSS detector](../../../src-tauri/src/media_tools.rs)

After the build has emitted rusty_h264-encoder and rusty_h264-common, repeat the elementary encode with `experiments/rust-native-assessment/remade/codec-check.sh`. It independently probes the stream, decodes with fatal errors enabled, verifies the frame count and measures PSNR.

## Reproduce

From the repository root on the Intel Mac, place the published crate and build products under the ignored Cargo target directory. The two binaries use the selected Rust toolchain, the crate's Cargo.lock and up to two build jobs. The test uses system FFmpeg/FFprobe for the reference, fixture preparation, and independent decode checks. It also uses jq.

```sh
repo="$(pwd)"
work="$repo/target/rust-assessment/remade"
mkdir -p "$work/cargo-home" "$work/build" "$work/tmp"
cd "$work"
curl -fLSs https://static.crates.io/crates/rff-cli/rff-cli-0.4.6.crate -o rff-cli-0.4.6.crate
printf '%s  %s\n' 506ee4c539f6779047fca8f7a48feccd64749ef58657cb1700f4ce1ef699d68e rff-cli-0.4.6.crate | LC_ALL=C shasum -a 256 -c -
tar -xzf rff-cli-0.4.6.crate
CARGO_HOME="$work/cargo-home" CARGO_TARGET_DIR="$work/build" TMPDIR="$work/tmp" \
    CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo +stable build --locked -j 2 \
    --manifest-path rff-cli-0.4.6/Cargo.toml --bin rff --bin rffprobe
"$repo/experiments/rust-native-assessment/remade/check.sh" \
    "$work/build/debug/rff" "$work/build/debug/rffprobe"
```

Each invocation prints its fresh evidence directory. Files with the `.exit` extension contain the individual program's exit code. The original production export arguments are in [export.rs](../../../src-tauri/src/export.rs). The reference files are the [Radeon native frame](../../desktop-rc1/macos/frame-30.png), the [RC movie](../../desktop-rc1/macos/coral-4k.mp4) and [two-second source audio](../../slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a).

The 4K pipe test repeats the native frame six times. The separate small encode uses all 60 moving frames of the RC movie and both audio channels. Probing, frame counts, audio metadata and full decoding test distinct parts of the media path. The unoptimized build tests compatibility. Runtime throughput and GPU encoding are outside these measurements.
