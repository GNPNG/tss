#!/bin/sh
# Rebuild with Basilisk qcc, CImg 3.6.0, librsvg, ImageMagick and zlib.
set -eu
export LC_ALL=C OMP_NUM_THREADS=1
: "${BASILISK:?Set BASILISK to the Basilisk src directory containing qcc}"
: "${CIMG_INCLUDE:?Set CIMG_INCLUDE to the directory containing CImg.h}"
if [ "$#" -ne 1 ]; then
    echo "Usage: reproduce.sh NEW_OUTPUT_DIRECTORY" >&2
    exit 2
fi
src=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
study="$src/../slide-look-study"
mkdir "$1"
out=$(CDPATH= cd -- "$1" && pwd)
cd "$out"
mkdir violet cyan
cp "$study/solver.c" .
"$BASILISK/qcc" -O2 -Wall -disable-dimensions solver.c -o solver -lm
"${CXX:-c++}" -std=c++17 -O2 "$src/geometry.cpp" -o geometry
"${CXX:-c++}" -std=c++17 -O2 -I"$CIMG_INCLUDE" "$src/verify.cpp" -lz -o verify
"${CXX:-c++}" -std=c++17 -O2 -I"$CIMG_INCLUDE" "$study/pack-world-cache.cpp" -lz -o pack
for pose in front rear; do
    if [ "$pose" = front ]; then channel=cyan; else channel=violet; fi
    ./geometry "$pose" > "$pose.svg"
    rsvg-convert -o "$pose.png" "$pose.svg"
    magick "$pose.png" -colorspace Gray -depth 8 "$pose.pgm"
    ./solver "$pose.pgm" "$channel" "$channel" 96 9 --world > "$pose.stdout" 2> "$pose.log"
    awk -f "$src/check-phase.awk" "$pose.log"
done
./verify --solve cyan
./pack . coral-release.rg8.gz
./verify --cache coral-release.rg8.gz
