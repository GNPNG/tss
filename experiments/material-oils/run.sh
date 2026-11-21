#!/bin/sh
# Run from any directory with an existing Basilisk solver built from the shared
# solver.c, a CImg 3.6.1 include directory, and a fresh output directory.
set -eu
: "${SOLVER:?Set SOLVER to the native Basilisk slide solver}"
: "${CIMG_INCLUDE:?Set CIMG_INCLUDE to the directory containing CImg.h}"
r=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
d=$1
mkdir "$d"
d=$(cd "$d" && pwd)
study=$r/experiments/slide-look-study
oils=$r/experiments/material-oils
mkdir "$d/grade20" "$d/grade1000" "$d/assets" "$d/evidence"
"${CXX:-c++}" -std=c++17 -O2 "$study/geometry-export.cpp" -o "$d/geometry"
"${CXX:-c++}" -std=c++17 -O2 -I"$CIMG_INCLUDE" "$study/pack-world-cache.cpp" -lz -o "$d/pack"
"${CXX:-c++}" -std=c++17 -O2 -I"$CIMG_INCLUDE" "$oils/measure.cpp" -o "$d/measure"

for slide in cyan violet; do
    "$d/geometry" "$slide" --world > "$d/$slide.svg"
    rsvg-convert -w 1024 -h 1024 -o "$d/$slide.png" "$d/$slide.svg"
    magick "$d/$slide.png" -colorspace Gray -depth 8 "pgm:$d/$slide.pgm"
done

solve() {
    label=$1; slide=$2; material=$3; folder=$4; states=$5; level=$6
    node "$oils/solve.mjs" "$SOLVER" "$d/$slide.pgm" "$d/$folder" "$material" "$states" "$level" "$slide" \
        > "$d/$label.stdout" 2> "$d/$label.log"
}
for grade in 20 1000; do
    solve "front$grade" cyan "pdms-water-${grade}cst" "grade$grade/cyan" 96 7
    solve "rear$grade" violet "pdms-water-${grade}cst" "grade$grade/violet" 96 7
    "$d/pack" "$d/grade$grade" "$d/assets/pdms-water-${grade}cst.rg8.gz"
done
node "$oils/verify.mjs" "$d" "$d/measure" "$d/assets" "$d/evidence"
