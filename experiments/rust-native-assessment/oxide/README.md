# OxideAV release tests

The two native Rust MP4 readers extracted every compressed sample with the same contents and timing as FFprobe in three TSS fixtures. Neither reader supplied the BT.709 signal required by the release check. The separate Intel Mac hardware trial ended before VideoToolbox accepted a session.

## MP4 and M4A

The [Rust 1.88 fixture run](results.json) uses [oxideav-mp4 0.0.10](https://docs.rs/crate/oxideav-mp4/0.0.10) and oxideav-core 0.1.41. The [second fixture run](latest-results.json) uses [oxideav-mp4 0.0.11](https://docs.rs/crate/oxideav-mp4/0.0.11) and oxideav-core 0.1.43 on Rust 1.100. The latter releases require Rust 1.95. Both use the same Rust source and FFprobe 9.0.1 reference. Each JSON identifies the input by path and SHA-256. Comparison is in demux order within each stream.

| Feature / property / behaviour | Legacy implementation | Proposed replacement | Preserved? | Notes / open questions |
|---|---|---|---|---|
| Stream discovery, codec, dimensions and audio format | FFprobe identified all five streams in the two 4K MP4s and separate M4A | Both OxideAV releases identified two H.264 streams at 3840 × 2160 and three stereo AAC streams at 48 kHz | Yes | Fixtures came from the Mac coral and Windows oil-and-star RCs and the source M4A. |
| Packet count and contents | FFprobe compressed packet SHA-256 | Both releases matched 120 H.264 and 285 AAC packets | Yes | Exactly 405 pairwise matches. These are compressed samples. |
| Packet PTS, DTS, duration, track time base and start tick | FFprobe timestamps in stream ticks | Both releases matched all packets, including reordered video timestamps and negative priming timestamps | Yes | The test compares each field independently. |
| Video duration | 30,720 ticks per stream | Both reported 30,720 | Yes | Both MP4s. |
| Audio presentation duration | 96,000 ticks | Both reported 97,024 ticks on all three AAC streams | No | Difference of one 1,024-sample AAC frame, 21.33 ms at 48 kHz. Matching packet PTS does not resolve the presentation edit. |
| Decoded pixel format signal | yuv420p on both videos | 0.0.10 returned no value. 0.0.11 returned Yuv420P | Partial | Requires the newer compiler for the tested 0.0.11 stack. |
| Colour primaries, transfer, matrix and range | BT.709 / BT.709 / BT.709 / limited | Both returned CICP 2 / 2 / 2 and unspecified range | No | The RC colour verifier still needs the bitstream signal. |
| Decode, seek, metadata outside these checks, damaged/fragmented media, other imports, encoding and muxing | FFmpeg and FFprobe | This fixture probe only exercises the sequential MP4 demux API | Unknown | Throughput, memory use, audio after priming, and encoded image quality require separate measurements. |

From the repository root, with Cargo, FFprobe, jq and shasum available:

```sh
experiments/rust-native-assessment/oxide/check.sh project
experiments/rust-native-assessment/oxide/check.sh latest
```

## Apple hardware bridge

The [project-compatible bridge 0.0.4](https://github.com/OxideAV/oxideav-videotoolbox/tree/d754dfe0189b05e1710782dedda36b5cf7b209a5) maps `hardware=require` to Apple's hardware requirement at session creation. The [0.0.5 bridge](https://github.com/OxideAV/oxideav-videotoolbox/tree/9c37dfe94b616699d5ac56606d42c9d0cf1e54eb) retains that path and requires Rust 1.95. The public encoder hides its session. The test build adds a [single observation call](hardware/observation.patch) to report the session's actual hardware flag and encoder ID after successful flush. The host framework supplies the encoder. [Apple hardware requirement](https://developer.apple.com/documentation/videotoolbox/kvtvideoencoderspecification_requirehardwareacceleratedvideoencoder)

On Intel macOS 15.3.1, the 0.0.4 H.264 factory returned `kVTInvalidSessionErr` (−12903) on creation under a single device reservation. Three 320 × 180 I420 frames from the coral RC were ready. The output stayed at zero bytes, and the code never reached the property query. Neither the hardware encoder identity nor encoding throughput is established. The result alone does not identify which session parameter or host condition caused the error. [Recorded test result](hardware/result.json)

`hardware/prepare.sh` verifies the upstream registry archive hash, applies the observation patch, builds with the 1.88 toolchain and Apple SDK, and stages three frames by software decoding. To reproduce the device test on an Intel Mac, pass the prepared `target/rust-assessment/oxide/build/debug/tss-oxide-vt-smoke` binary, the absolute staged `coral-320.i420` path and a new absolute output filename to the existing GPU-job guard, with the device reservation and 45-second timeout. A passing probe requires both the actual hardware property and three emitted packets.

The separate [oxideav-nvidia source](https://github.com/OxideAV/oxideav-nvidia/tree/1b2cc816cb43a9f14d36e2328bb5c5989d93b2b0) gates its implementation on Linux. The Windows RC served as MP4 input to the CPU test. Its encoder remains a separate qualification target.
