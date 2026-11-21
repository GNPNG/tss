#!/bin/sh
# Build the native helpers alongside the Cargo executable used by the editor.
set -eu
cd "$(dirname "$0")/.."
export LC_ALL=C
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/src-tauri/target}"
case "$1" in
    dev)   profile=debug ;;
    build) profile=release ;;
    *) echo 'Expected dev or build' >&2; exit 2 ;;
esac
shift
mkdir -p "$CARGO_TARGET_DIR/$profile"
bin=$(cd "$CARGO_TARGET_DIR/$profile" && pwd)
if [ "$(uname -s)" = Darwin ]; then
    : "${CIMG_INCLUDE:?Set CIMG_INCLUDE to the directory containing CImg.h}"
    [ -f "$CIMG_INCLUDE/CImg.h" ] || { echo "Missing $CIMG_INCLUDE/CImg.h" >&2; exit 1; }
    export CC="${CC:-/usr/bin/clang}" CXX="${CXX:-/usr/bin/clang++}"
    stage=$(mktemp -d "$bin/.solver.XXXXXXXX")
    trap 'rm -rf "$stage"' EXIT
    sh experiments/live-slide-solver/build-worker.sh "$stage/tss-slide-solver"
    mv -f "$stage/tss-slide-solver" "$bin/tss-slide-solver"
    set -- --features live-solver "$@"
fi
go -C performer build -trimpath -o "$bin/tss-performer" .
case "$profile" in
    debug)   cargo run --locked --manifest-path src-tauri/Cargo.toml "$@" ;;
    release) cargo build --locked --manifest-path src-tauri/Cargo.toml --release --features tauri/custom-protocol "$@" ;;
esac
