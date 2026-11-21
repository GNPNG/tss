#!/bin/sh
set -eu
dest=${1:?Use a new directory for the two visual clips}
mkdir "$dest"
work=$(mktemp -d "${TMPDIR:-/tmp}/tss-aperture.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir "$work/star" "$work/matte"
cc -std=c11 -Wall -Wextra -Werror "$(dirname "$0")/fixture.c" $(pkg-config --cflags --libs cairo) -lm -o "$work/fixture"
"$work/fixture" star "$work/star"
"$work/fixture" matte "$work/matte"
ffmpeg -v error -nostdin -framerate 24 -i "$work/star/%03d.png" -f lavfi -i color=c=0x00ff00:s=960x540:r=24 \
  -filter_complex '[1:v][0:v]overlay=shortest=1:format=gbrp,format=gbrp' \
  -c:v libx264rgb -threads:v 2 -crf 0 -preset fast -movflags +faststart "$dest/star-green.mp4"
ffmpeg -v error -nostdin -framerate 24 -i "$work/star/%03d.png" \
  -c:v png -pix_fmt rgba -threads:v 2 "$dest/star-alpha.mov"
ffmpeg -v error -nostdin -framerate 24 -i "$work/matte/%03d.png" \
  -c:v png -pix_fmt rgba -threads:v 2 "$dest/moving-matte.mov"
