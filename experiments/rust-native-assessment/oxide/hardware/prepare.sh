#!/bin/sh
# Build the instrumented probe and stage input. All media operations here run on CPU.
set -eu
export LC_ALL=C
study=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$study/../../../.." && pwd)
artifacts="$repo/target/rust-assessment/oxide"
mkdir -p "$artifacts"
vendor="$artifacts/instrumented/oxideav-videotoolbox-0.0.4"
mkdir -p "$(dirname "$vendor")"
archive="$artifacts/videotoolbox-0.0.4.crate"
curl -fsSL --output "$archive" https://static.crates.io/crates/oxideav-videotoolbox/oxideav-videotoolbox-0.0.4.crate
printf '%s  %s\n' 84caf65b030a415bf2ce791917390bc415e7c70194a100907a49109af948e650 "$archive" | shasum -c - -a 256
tar xzf "$archive" -C "$(dirname "$vendor")"
patch -d "$vendor" -p1 < "$study/observation.patch"
export CARGO_HOME="$artifacts/cargo-home" CARGO_TARGET_DIR="$artifacts/build"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
export CC="$(xcrun --find clang)"
cargo +1.88.0 build --manifest-path "$study/Cargo.toml" --locked -j1 \
  --config "patch.crates-io.oxideav-videotoolbox.path='$vendor'"
ffmpeg -nostdin -v error -hwaccel none -threads 1 -i "$repo/experiments/desktop-rc1/macos/coral-4k.mp4" \
  -an -filter_threads 1 -vf scale=320:180 -frames:v 3 -pix_fmt yuv420p -f rawvideo -y "$artifacts/coral-320.i420"
