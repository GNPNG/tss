#!/bin/sh
set -eu
case "$#" in 1) ;; *) echo "Usage: build-worker.sh NEW_EXECUTABLE" >&2; exit 2;; esac
repo=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
out_dir=$(CDPATH='' cd -- "$(dirname -- "$1")" && pwd)
out="$out_dir/$(basename -- "$1")"
test ! -e "$out" || { echo "Destination already exists: $out" >&2; exit 2; }
: "${BASILISK:=$repo/experiments/basilisk-study/vendor/patched/basilisk/src}"
export BASILISK
test -x "$BASILISK/qcc" || { echo "Set BASILISK to the matching host's built Basilisk src directory" >&2; exit 2; }
case $(uname -s) in Darwin) : "${CC99:=/usr/bin/clang -std=c99 -D_DARWIN_C_SOURCE}"; export CC99;; esac
export LC_ALL=C
dir=$(mktemp -d "${TMPDIR:-/tmp}/tss-live-build.XXXXXXXX")
trap 'rm -rf -- "$dir"' EXIT
cp "$repo/experiments/slide-look-study/solver.c" "$dir/solver.c"
(cd "$dir" && "$BASILISK/qcc" -O2 -Wall -disable-dimensions solver.c -o solver -lm)
cp "$dir/solver" "$out"
