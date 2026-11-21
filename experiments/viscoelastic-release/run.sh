#!/bin/sh
set -eu
: "${BASILISK:?Set BASILISK to the pinned Basilisk src directory}"
out=${1:?Usage: run.sh new-output-directory}
here=$(CDPATH='' cd -- "$(dirname "$0")" && pwd)
mkdir "$out"
cd "$out"
export OMP_NUM_THREADS=1 LC_ALL=C
cp "$BASILISK/test/viscodrop.c" .
test "$(openssl dgst -sha256 viscodrop.c | awk '{print $NF}')" = a026df0be42402bfdaa3862bb21864d3a19c52ad331ac64faf155cc9fa48b6ed
"$BASILISK/qcc" -O2 -Wall viscodrop.c -o stock -lm
./stock > stock-contour.dat 2> stock.dat
patch -p1 < "$here/stop-shear.patch"
"$BASILISK/qcc" -O2 -Wall viscodrop.c -o release -lm
for material in oldroyd-b newtonian; do
    mkdir "$material"
    (cd "$material" && ../release "$material" > end.dat 2> deformations.csv)
done
Rscript "$here/analyze.R" . "$BASILISK/test" .
