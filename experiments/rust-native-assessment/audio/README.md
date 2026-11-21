# Symphonia audio import study

Symphonia 0.6.1 decoded MP3, AAC-LC in M4A and ALAC in M4A on the CPU. The AAC-LC decoder reconstructed the waveform accurately. Its default MP4 reader exposed codec priming and tail padding as extra audio on TSS's own cue fixture. That shifts the cues by 21.33 ms. An integration would need to honor the container's edit list before using decoded samples to position effects on the song timeline. The test records the offset separately, leaving the candidate output intact.

## Measurements

The input was the checked-in [two-slide cue recording](../../slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a) used in RC.1 testing. SHA-256 is `0342a6d244a314bca1abd0d81e160210373f23ebcb7955280a33c51ddb6a3f42`. FFmpeg 9.0.1 created a 192 kb/s MP3 and a losslessly re-encoded ALAC variant from the same recording, then independently decoded each of the three encoded files to interleaved 32-bit float PCM. The MP3 and ALAC are controlled format variants of this recording. All three references contain 96,000 stereo frames at 48 kHz.

| Input | Frames from Symphonia | Reference frames | SNR at the same timestamps | Peak absolute difference |
|---|---:|---:|---:|---:|
| Original AAC-LC / M4A | 97,280 | 96,000 | −0.289 dB | 0.27588178 |
| Derived MP3 | 96,000 | 96,000 | 124.571 dB | 0.00000037 |
| Derived ALAC / M4A | 96,000 | 96,000 | Identical PCM values | 0 |

The Intel Mac (i7-9750H, macOS 15.3.1) and Windows x64 laptop (`flip`, native MSVC binary) produced the same frame counts, SNR and peak errors shown above. Both machines ran the decoder locally against the same fixture bytes and reference PCM generated on the Mac. SNR is 10 log10 of total reference energy divided by total squared sample error over the common frame count, across both channels. Differences are expressed on the float PCM scale where 1.0 is full scale. The trials cover three two-second stereo recordings at 48 kHz. [Raw measurements from both hosts](measurements.txt)

FFprobe reports a 1,024-frame start trim and a 256-frame end trim on the AAC fixture. Comparing the 96,000 reference frames against the Symphonia waveform starting at frame 1,024 gives **132.031 dB SNR**, peak difference **0.00000010**. The ISO MP4 reader's source explains that it derives duration from media samples and does not apply track edits in this release. Read the actual edit list for each recording to determine its offsets. [Symphonia v0.6.1 MP4 reader](https://github.com/pdeljanov/Symphonia/blob/v0.6.1/symphonia-format-isomp4/src/demuxer.rs#L300-L315)

## Reproduce

Run this from the TSS checkout.

```sh
sh experiments/rust-native-assessment/audio/run.sh
```

The command uses Rust 1.88.0, an isolated release target, two Cargo build jobs, and one FFmpeg codec thread. FFmpeg and FFprobe are test peers. The probe itself uses Symphonia for demuxing and decoding with MP3, AAC, ALAC, ISO MP4, ID3v2 and SIMD enabled. Test media, PCM references and build products go under `target/rust-assessment/audio`.

To repeat the Windows leg, copy this study's Cargo manifest, lock file and `src` directory to a new scratch directory alongside the original M4A, the two derived files and the three PCM references from the preceding command. In an x64 Visual Studio developer shell with Rust 1.88 on PATH, run:

```cmd
cargo build --release --locked -j 1
target\release\tss-symphonia-comparison.exe two-slide-cues.m4a two-slide-cues.m4a.f32le 1024
target\release\tss-symphonia-comparison.exe derived.mp3 derived.mp3.f32le
target\release\tss-symphonia-comparison.exe derived-alac.m4a derived-alac.m4a.f32le
```
