#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
mkdir -p target/rust-assessment/inventory
export CARGO_TARGET_DIR="$PWD/target/rust-assessment/kernels"
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 build --locked -j 1 --manifest-path experiments/rust-native-assessment/native-kernels/Cargo.toml
work="$(mktemp -d "$PWD/target/rust-assessment/inventory/kernels.XXXX")"
mkdir -p "$work/optics"
c++ -std=c++17 -O2 experiments/phrase-triggers/match.cpp $(pkg-config --cflags --libs libhs) -o "$work/vectorscan"
"$work/vectorscan" > "$work/vectorscan.csv"
target/rust-assessment/kernels/debug/tss-rust-kernel-assessment cues > "$work/rust.csv"
diff -u experiments/phrase-triggers/fixture.csv "$work/vectorscan.csv"
diff -u "$work/vectorscan.csv" "$work/rust.csv"
rustc +1.88.0 --edition=2021 live-wheel/build.rs -o "$work/generate"
(cd live-wheel && OUT_DIR="$work/optics" "$work/generate" > "$work/build-log")
for shaderfile in "$work"/optics/*.frag; do
  name=$(basename "$shaderfile" .frag)
  spirv-val --target-env vulkan1.1 "$work/optics/$name.spv"
  if target/rust-assessment/kernels/debug/tss-rust-kernel-assessment shader "$shaderfile" > "$work/$name.log" 2>&1 && spirv-val --target-env vulkan1.1 "$work/optics/$name.naga.spv" >> "$work/$name.log" 2>&1; then
    printf '%s\tpass\n' "$name"
  else
    printf '%s\tfail\t' "$name"
    head -c 240 "$work/$name.log"
    printf '\n'
  fi
done
cat "$work/rust.csv"
