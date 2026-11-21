#!/bin/sh
# Encode two registered native GPU readbacks with their original sRGB still.
set -eu
input=${1:?Usage: encode.sh capture-directory new-review-directory}
out=${2:?Usage: encode.sh capture-directory new-review-directory}
test -s "$input/front-20.rgba.gz"
test -s "$input/front-1000.rgba.gz"
mkdir "$out"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/tss-oil-film.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
font="$(dirname "$0")/../../live-wheel/assets/type/SourceSans3-Regular.otf"
magick -size 1280x48 xc:'#211e29' -font "$font" -pointsize 24 -fill '#fff8f2' \
    -annotate +16+32 'Front oil   20 cSt' -annotate +656+32 'Front oil   1,000 cSt' \
    -depth 8 "PNG24:$tmp/labels.png"
mkfifo "$tmp/light" "$tmp/heavy"
gzip -dc "$input/front-20.rgba.gz" >"$tmp/light" & left=$!
gzip -dc "$input/front-1000.rgba.gz" >"$tmp/heavy" & right=$!
ffmpeg -nostdin -loglevel error -n -filter_complex_threads 2 \
    -f rawvideo -pixel_format rgba -video_size 640x360 -framerate 24 -i "$tmp/light" \
    -f rawvideo -pixel_format rgba -video_size 640x360 -framerate 24 -i "$tmp/heavy" \
    -loop 1 -framerate 24 -i "$tmp/labels.png" \
    -filter_complex '[0:v][1:v]hstack[oil];[2:v]format=rgba[label];[label][oil]vstack=shortest=1,split[rgb][movie];[rgb]select=eq(n\,48),format=rgb24[still];[movie]scale=out_color_matrix=bt709:out_range=tv,format=yuv444p10le,colorspace=iall=bt709:itrc=srgb:irange=tv:all=bt709:range=tv:format=yuv420p[video]' \
    -map '[video]' -c:v libx264 -threads 3 -preset fast -crf 16 -color_range tv \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 -movflags +faststart "$out/front-oils.mp4" \
    -map '[still]' -frames:v 1 -update 1 "$out/at-two-seconds.png"
wait "$left"
wait "$right"
test "$(ffprobe -v error -select_streams v:0 -show_entries stream=nb_frames -of csv=p=0 "$out/front-oils.mp4")" = 96
cp "$input/capture.json" "$out/capture.json"
