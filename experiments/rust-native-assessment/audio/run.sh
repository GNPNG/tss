#!/bin/sh
# Run with sh from any directory. All audio outputs are local fixtures.
set -eu
study=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH='' cd -- "$study/../../.." && pwd)
out="$repo/target/rust-assessment/audio"
fixture="$repo/experiments/slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a"
mkdir -p "$out"
cargo +1.88.0 build --jobs 2 --release --locked --manifest-path "$study/Cargo.toml" --target-dir "$out/cargo"
ffmpeg -nostdin -v error -y -threads 1 -i "$fixture" -map 0:a:0 -threads 1 -c:a libmp3lame -b:a 192k "$out/derived.mp3"
ffmpeg -nostdin -v error -y -threads 1 -i "$fixture" -map 0:a:0 -threads 1 -c:a alac "$out/derived-alac.m4a"
for input in "$fixture" "$out/derived.mp3" "$out/derived-alac.m4a"; do
    pcm="$out/$(basename "$input").f32le"
    ffmpeg -nostdin -v error -y -threads 1 -i "$input" -map 0:a:0 -threads 1 -f f32le "$pcm"
    if [ "$input" = "$fixture" ]; then
        # Use the edit-list trim reported by ffprobe to diagnose the real fixture.
        skip=$(ffprobe -v error -select_streams a:0 -read_intervals '%+#1' -show_packets \
            -show_entries packet_side_data=skip_samples -of default=noprint_wrappers=1:nokey=1 "$input" | tail -1)
        "$out/cargo/release/tss-symphonia-comparison" "$input" "$pcm" "$skip"
    else
        "$out/cargo/release/tss-symphonia-comparison" "$input" "$pcm"
    fi
done
