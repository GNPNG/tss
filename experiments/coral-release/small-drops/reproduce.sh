#!/bin/sh
# First build the original coral experiment with ../reproduce.sh.
set -eu
export LC_ALL=C OMP_NUM_THREADS=1
: "${CIMG_INCLUDE:?Set CIMG_INCLUDE to the directory containing CImg.h}"
if [ "$#" -ne 2 ]; then
    echo "Usage: reproduce.sh ORIGINAL_CORAL_REPRODUCTION_DIR NEW_OUTPUT_DIR" >&2
    exit 2
fi
src=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
original=$(CDPATH= cd -- "$1" && pwd)
mkdir "$2"
cd "$2"
mkdir cyan
ln -s "$original/violet" violet
"${CXX:-c++}" -std=c++17 -O2 "$src/geometry.cpp" -o geometry
./geometry > front.svg
rsvg-convert -o front.png front.svg
magick front.png -colorspace Gray -depth 8 front.pgm
"$original/solver" front.pgm cyan cyan 96 9 --world > front.stdout 2> front.log
awk -f "$src/../check-phase.awk" front.log
"$original/verify" --solve cyan > source-events.csv
"$original/pack" . small.rg8.gz
"$original/verify" --cache small.rg8.gz > optical-events.csv
ruby "$src/extract-front.rb" small.rg8.gz coral-small-front.r8.gz
"${CXX:-c++}" -std=c++17 -O2 -I"$CIMG_INCLUDE" "$src/check-matches.cpp" -lz -o check-matches
./check-matches "$src/matched-sites.csv" cyan coral-small-front.r8.gz
