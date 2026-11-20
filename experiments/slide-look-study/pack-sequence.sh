#!/bin/bash
# Convert matched Basilisk states to a lossless GPU input, one time step per frame.
set -euo pipefail
if [[ $# != 4 ]]; then
    echo 'usage: pack-sequence.sh PACK-FIELDS SOLVER-OUT FRAME-COUNT OUTPUT.mkv' >&2
    exit 2
fi
pack=$1
states=$2
count=$3
dest=$4
work=$(mktemp -d)
trap 'rm -f "$work/frame.ppm" "$work/fields.mkv"; rmdir "$work"' EXIT
for ((i=0; i<count; i++)); do
    id=$(printf '%03d' "$i")
    "$pack" "$states/violet/$id.pgm" "$states/cyan/$id.pgm" 1024 composite "$work/frame.ppm"
    cat "$work/frame.ppm"
done | ffmpeg -hide_banner -loglevel error -nostdin -f image2pipe -framerate 24 -c:v ppm -i pipe:0 \
    -c:v ffv1 -level 3 -g 1 -pix_fmt gbrp16le -threads 2 -color_range pc \
    -colorspace rgb -color_primaries bt709 -color_trc iec61966-2-1 "$work/fields.mkv"
actual=$(ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 "$work/fields.mkv")
[[ "$actual" == "$count" ]] || { echo "Expected $count states, encoded $actual" >&2; exit 1; }
# Refuse to overwrite a previous run.
( set -o noclobber; cat "$work/fields.mkv" > "$dest" )
