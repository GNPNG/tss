#!/bin/sh
set -eu
export LC_ALL=C

study=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$study/../../.." && pwd)
artifacts="$repo/target/rust-assessment/oxide"
export CARGO_HOME="$artifacts/cargo-home"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
mkdir -p "$artifacts"
mode="${1:-project}"
case "$mode" in
    project) manifest="$study/Cargo.toml"; compiler=1.88.0 ;;
    latest) manifest="$study/current/Cargo.toml"; compiler=stable ;;
    *) echo "usage: check.sh [project|latest]" >&2; exit 2 ;;
esac
reports="$artifacts/reports/$mode"
mkdir -p "$reports"
CARGO_TARGET_DIR="$artifacts/build-$mode"
export CARGO_TARGET_DIR
cargo "+$compiler" build --locked -j1 --manifest-path "$manifest"
cd "$repo"

probe() {
    label="$1"
    input="$2"
    "$CARGO_TARGET_DIR/debug/tss-oxide-probe-check" "$input" >"$reports/$label.oxide.json"
    ffprobe -v error -show_packets -show_data_hash sha256 \
      -show_entries 'stream=index,codec_name,width,height,pix_fmt,sample_rate,channels,color_space,color_primaries,color_transfer,color_range,time_base,duration_ts,start_pts:packet=stream_index,pts,dts,duration,data_hash' \
      -of json "$input" >"$reports/$label.ffprobe.json"
    jq -n --slurpfile probe_file "$reports/$label.oxide.json" \
      --slurpfile ff_file "$reports/$label.ffprobe.json" -f "$study/compare.jq" >"$reports/$label.diff.json"
    jq -n --arg input "$input" --arg hash "$(shasum -a 256 "$input" | cut -d' ' -f1)" \
      --slurpfile streams "$reports/$label.diff.json" '{input:$input,sha256:$hash,streams:$streams[0]}'
}

probe mac experiments/desktop-rc1/macos/coral-4k.mp4 >"$reports/mac.summary.json"
probe windows experiments/desktop-rc1/windows/oil-and-star-4k.mp4 >"$reports/windows.summary.json"
probe audio experiments/slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a >"$reports/audio.summary.json"
jq -s . "$reports/mac.summary.json" "$reports/windows.summary.json" "$reports/audio.summary.json"
