# Two-cue 4K Windows export

The native Windows editor decoded a two-second stereo M4A, rendered the selected Violet / Blue wheel on the GTX 1650 Ti Max-Q, and passed 60 uncompressed 3840 × 2160 frames to FFmpeg. The original audio contains a decaying 220 Hz tone at 0.25–0.75 s and a 330 Hz tone at 1.10–1.60 s. A rear material pull follows the first cue and the front glass rotates through one quarter turn during the second. [Original M4A](two-slide-cues.m4a) · [Cue timings](two-slide-cues.vtt) · [Finished H.264/AAC video](two-slide-cues.mp4)

Both output streams start at zero and run for 2.000 s. Video is 30 fps H.264, limited-range Rec.709 YUV 4:2:0. Audio is 48 kHz stereo AAC. The GPU guard recorded 22 samples over launch, export and projector checks, peaking at 35% utilization, 231 MiB allocated, 58 °C and 18.99 W. [Stream metadata](streams.json) · [GPU telemetry](gpu-public.csv)

At frame 42 (1.400 s), the application saved the selected wheel's 4K optical readback before encoding. Applying the same sRGB-to-Rec.709 conversion and comparing it with decoded H.264 frame 42 produced 55.98 dB average YUV PSNR. The luma channel measured 56.26 dB. Comparing decoded mono audio with decoded source audio gave correlations of 0.9941 during the rear cue and greater than 0.99999 during the front cue. The three tested silent windows had zero RMS in both. [Native frame 42](frame-0042.png) · [PSNR report](frame-0042-psnr.txt) · [Audio comparisons](audio-cues.json)

The Windows WebView2 run verified seven wheel options, the macro deck, the feedback controls and the projector controls after audio import. The projector opened and closed on the one attached display and presented 18 frames while the transport was paused. Both audio routes were muted. The runtime exposed device enumeration, AudioContext.setSinkId and HTMLMediaElement.setSinkId. The explicit selectAudioOutput picker was unavailable and the session enumerated zero individually addressable output devices beyond system endpoints. The measured age is for a paused source frame. A second physical display and independently routed speakers were outside this hardware run. [Runtime result](two-slide-cues.webview.json)

To repeat the integrated test, launch the Windows editor with `--verify-wheel-export <absolute-path-to-two-slide-cues.m4a> <absolute-path-to-a-fresh-mp4>`. With the same process set `TSS_WHEEL_EXPORT_PROOF` to a new PPM path and `TSS_WHEEL_EXPORT_PROOF_FRAME=42`. The launcher uses the editor's asset grants and importer, native export command and projector controls. A sibling WebVTT is imported through the normal audio-import path. The diagnostic supplies the two scripted gesture series at the times above. [Verification UI](../../../../../../WebAssets/verify-wheel-export.js)

Public telemetry uses a device ordinal within each run. All sampled times, device indices and numeric measurements retain their recorded values.

An 8,192-sample Hann-windowed FFmpeg FFT found the same two peak frequencies in the separately decoded M4A and exported AAC. At 0.35 s the fitted peak was 219.963 Hz in each; at 1.20 s it was 330.083 Hz in each. The fit uses the log amplitudes of the strongest FFT bin and its two neighbors. The nominal bin width is 5.859 Hz. [Spectrum measurements](cue-spectrum.csv) · [FFmpeg FFT fixture](cue-spectrum.cpp)

To reproduce, run in this directory with the FFmpeg CLI and libavutil development headers installed.

```sh
ffmpeg -i two-slide-cues.m4a -map 0:a:0 -ac 1 -ar 48000 -f f32le cue-source.f32le
ffmpeg -i two-slide-cues.mp4 -map 0:a:0 -ac 1 -ar 48000 -f f32le cue-export.f32le
c++ -std=c++17 -O2 cue-spectrum.cpp $(pkg-config --cflags --libs libavutil) -o cue-spectrum
./cue-spectrum cue-source.f32le cue-export.f32le
```
