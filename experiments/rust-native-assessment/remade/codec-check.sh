#!/bin/bash
# Compile only a small lib consumer, reusing rff-cli's existing Cargo artifacts.
set -eu
set -o pipefail
export LC_ALL=C RAYON_NUM_THREADS=2
root="$(cd "$(dirname "$0")/../../.." && pwd)"
work="$root/target/rust-assessment/remade"
out="$(mktemp -d "$work/codec.XXXXXX")"
search_dirs=()
while IFS= read -r directory; do
  search_dirs+=(-L "dependency=$directory")
done < <(find "$work/build" -name '*.rlib' -exec dirname {} \; | sort -u)
common="$(find "$work/build" -name 'librusty_h264_common-*.rlib' -print -quit)"
encoder="$(find "$work/build" -name 'librusty_h264_encoder-*.rlib' -print -quit)"
rustc +stable --edition=2021 "$root/experiments/rust-native-assessment/remade/encode-frames.rs" \
    "${search_dirs[@]}" --extern "rusty_h264_common=$common" \
    --extern "rusty_h264_encoder=$encoder" -o "$out/encode"
cd "$out"
printf 'Codec evidence directory: %s\n' "$out"
ffmpeg -hide_banner -v error -threads 1 -i "$root/experiments/desktop-rc1/macos/coral-4k.mp4" \
    -map 0:v:0 -filter_threads 1 -vf scale=160:90 -pix_fmt yuv420p -threads:v 1 -f rawvideo source.yuv
./encode < source.yuv > result.h264
ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,pix_fmt,nb_read_frames \
    -of json result.h264 > streams.json
jq -e 'any(.streams[]; .codec_name=="h264" and .width==160 and .height==90 and .nb_read_frames=="60")' streams.json >/dev/null
ffmpeg -hide_banner -v error -xerror -threads 1 -r 30 -i result.h264 -fps_mode passthrough \
    -threads:v 1 -pix_fmt yuv420p -f rawvideo decoded.yuv
test "$(wc -c < decoded.yuv)" = "$(wc -c < source.yuv)"
ffmpeg -hide_banner -v info -f rawvideo -pix_fmt yuv420p -video_size 160x90 -i source.yuv \
    -f rawvideo -pix_fmt yuv420p -video_size 160x90 -i decoded.yuv \
    -filter_complex_threads 1 -filter_complex psnr -f null - 2> psnr.log
grep 'PSNR y:' psnr.log
