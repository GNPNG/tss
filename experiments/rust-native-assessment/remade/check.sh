#!/bin/bash
# Probe the RC movie and replay the media command of ExportManager::start_with.
# Pass the candidate binaries explicitly so the system FFmpeg stays available.
set -eu
set -o pipefail
export LC_ALL=C RAYON_NUM_THREADS=2
root="$(cd "$(dirname "$0")/../../.." && pwd)"
rff="${1:?usage: check.sh /absolute/path/to/rff /absolute/path/to/rffprobe}"
rffprobe="${2:?supply an absolute rffprobe path}"
reference="$root/experiments/desktop-rc1/macos/coral-4k.mp4"
native_frame="$root/experiments/desktop-rc1/macos/frame-30.png"
audio="$root/experiments/slide-look-study/approval/windows-live/gesture-controls/export/two-slide-cues.m4a"
mkdir -p "$root/target/rust-assessment/remade"
evidence="$(mktemp -d "$root/target/rust-assessment/remade/run.XXXXXX")"
cd "$evidence"
printf 'Evidence directory: %s\n' "$evidence"

# Save the original native sRGB frame as RGBA once. Six frames exercise the pipe
# without requiring another GPU job or allocating a raw movie on disk.
ffmpeg -hide_banner -v error -threads 1 -i "$native_frame" -frames:v 1 \
  -filter_threads 1 -threads:v 1 -pix_fmt rgba -f rawvideo frame.rgba
test "$(wc -c < frame.rgba)" -eq 33177600
export_args=(-hide_banner -loglevel error -y -f rawvideo -pixel_format rgba
  -video_size 3840x2160 -framerate 30 -i pipe:0 -i "$audio"
  -filter_threads 2 -threads:v 2 -x264-params rc-lookahead=8:sync-lookahead=0:threads=2
  -vf scale=out_color_matrix=bt709:out_range=tv,format=yuv444p10le,colorspace=iall=bt709:itrc=srgb:irange=tv:all=bt709:range=tv:format=yuv420p
  -color_range tv -color_primaries bt709 -color_trc bt709 -colorspace bt709
  -map 0:v:0 -map 1:a:0 -c:v libx264 -crf 18 -pix_fmt yuv420p
  -c:a aac -b:a 256k -movflags +faststart -shortest -f mp4)

run_export() {
  local program="$1" name="$2" i
  set +e
  (for ((i=0; i<6; i++)); do cat frame.rgba || exit; done) |
    "$program" "${export_args[@]}" "$name.mp4" > "$name.out" 2> "$name.err"
  local statuses=("${PIPESTATUS[@]}")
  set -e
  printf '%s\n' "${statuses[1]}" > "$name.exit"
}

run_checked() {
  local name="$1" status=0
  shift
  "$@" > "$name.out" 2> "$name.err" || status=$?
  printf '%s\n' "$status" > "$name.exit"
}

inspect_movie() {
  local path="$1"
  ffprobe -v error -count_frames -show_entries \
    stream=index,codec_name,width,height,pix_fmt,color_space,color_transfer,color_primaries,avg_frame_rate,duration,nb_read_frames,sample_rate,channels \
    -of json "$path"
}

run_export ffmpeg reference-export
test "$(cat reference-export.exit)" = 0
inspect_movie reference-export.mp4 > reference-export.json
jq -e 'any(.streams[]; .codec_name=="h264" and .width==3840 and .height==2160 and .color_transfer=="bt709" and .nb_read_frames=="6") and any(.streams[]; .codec_name=="aac" and .sample_rate=="48000" and .channels==2)' reference-export.json >/dev/null
run_export "$rff" candidate-export
run_checked candidate-version "$rff" -version
run_checked candidate-probe-version "$rffprobe" -v error -show_program_version -of json
run_checked candidate-probe "$rffprobe" -v error -show_streams -show_format -of json "$reference"

# Independent positive controls. These use RFF's documented CLI and do not
# replace any of the app's FFmpeg calls.
run_checked candidate-remux "$rff" -hide_banner -i "$reference" -c copy -y candidate-remux.mp4
ffmpeg -hide_banner -v error -threads 1 -i "$reference" -filter_threads 1 \
  -vf scale=160:90 -c:v libx264 -threads:v 1 -c:a copy control-input.mp4
run_checked candidate-encode "$rff" -hide_banner -i control-input.mp4 \
  -c:v h264 -c:a aac -y candidate-encode.mp4
for name in candidate-export candidate-remux candidate-encode; do
  if [[ -s "$name.mp4" ]]; then
    run_checked "$name-parse" inspect_movie "$name.mp4"
    run_checked "$name-decode" ffmpeg -v error -xerror -threads 1 \
      -i "$name.mp4" -map 0:v:0 -map 0:a:0 -f null -
  fi
done
stream_hashes() {
  local movie="$1" name="$2"
  ffmpeg -v error -xerror -threads 1 -i "$movie" -map 0:v:0 \
    -c:v rawvideo -threads:v 1 -f hash -hash sha256 - > "$name.video.sha256"
  ffmpeg -v error -xerror -threads 1 -i "$movie" -map 0:a:0 \
    -c:a pcm_s16le -f hash -hash sha256 - > "$name.audio.sha256"
}
if [[ -f candidate-remux-decode.exit && $(cat candidate-remux-decode.exit) == 0 ]]; then
  stream_hashes "$reference" reference
  stream_hashes candidate-remux.mp4 candidate-remux
fi
for name in *.exit; do printf '%s %s\n' "${name%.exit}" "$(cat "$name")"; done
