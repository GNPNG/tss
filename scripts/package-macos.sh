#!/bin/bash
# Build an Intel macOS .app and ZIP with Basilisk qcc and the CImg optics bridge.
# Usage: scripts/package-macos.sh v0.1.0-preview.1 /path/to/output
# TSS_TAURI_CLI can point to the official tauri executable installed by npm.
set -euo pipefail
export LC_ALL=C

fail() { echo "$*" >&2; exit 1; }
[[ $# == 2 && $1 =~ ^v[0-9][A-Za-z0-9.-]*$ ]] || fail "Usage: $0 <vTAG> <output directory>"
[[ $(uname -sm) == "Darwin x86_64" ]] || fail "Run the Intel package build on an x86_64 Mac."
tag=$1
mkdir -p "$2"
out=$(cd "$2" && pwd -P)
repo=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$repo"
app="The Sacred Swirls.app"
zip="The-Sacred-Swirls-$tag-macOS-x86_64.zip"
[[ ! -e "$out/$app" && ! -e "$out/$zip" ]] || fail "The output already contains this app or archive."

if [[ -n ${TSS_TAURI_CLI:-} ]]; then
  cli=("$TSS_TAURI_CLI")
elif command -v tauri >/dev/null; then
  cli=(tauri)
else
  cli=(cargo tauri)
fi
"${cli[@]}" --version
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repo/src-tauri/target"}
mkdir -p "$CARGO_TARGET_DIR"
CARGO_TARGET_DIR=$(cd "$CARGO_TARGET_DIR" && pwd -P)
helper_dir="$repo/src-tauri/target/mac-package"
mkdir -p "$helper_dir"
: "${BASILISK:?Set BASILISK to the built Basilisk src directory containing qcc}"
: "${CIMG_INCLUDE:?Set CIMG_INCLUDE to the directory containing CImg.h}"
[[ -x "$BASILISK/qcc" && -f "$CIMG_INCLUDE/CImg.h" ]] || fail "Missing qcc or CImg.h"
export BASILISK CIMG_INCLUDE
(
  cd "$helper_dir"
  cp "$repo/experiments/slide-look-study/solver.c" .
  CC99="${CC99:-/usr/bin/clang -std=c99 -D_DARWIN_C_SOURCE}" "$BASILISK/qcc" -O2 -Wall -disable-dimensions solver.c -o tss-slide-solver-x86_64-apple-darwin -lm
)
CGO_ENABLED=0 GOOS=darwin GOARCH=amd64 go -C performer build -trimpath -o "$helper_dir/tss-performer-x86_64-apple-darwin" .
cargo build --locked --release --features tauri/custom-protocol,live-solver --manifest-path src-tauri/Cargo.toml
"${cli[@]}" bundle --ci --bundles app --features tauri/custom-protocol,live-solver --config "$repo/src-tauri/tauri.package-macos.conf.json"

built="$CARGO_TARGET_DIR/release/bundle/macos/$app"
verify_bundle() {
  local contents="$1/Contents" exe name
  plutil -lint "$contents/Info.plist" >/dev/null
  exe=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$contents/Info.plist")
  for name in "$exe" tss-performer tss-slide-solver; do
    [[ -x "$contents/MacOS/$name" ]] || fail "Missing executable: $name"
    [[ $(lipo -archs "$contents/MacOS/$name") == x86_64 ]] || fail "Expected Intel executable: $name"
    codesign --verify --strict "$contents/MacOS/$name"
  done
  codesign --verify --deep --strict --verbose=2 "$1"
}
verify_bundle "$built"

stage=$(mktemp -d "$out/.tss-mac-package.XXXXXX")
trap 'rm -r "$stage"' EXIT
ditto -c -k --sequesterRsrc --keepParent "$built" "$stage/$zip"
ditto -x -k "$stage/$zip" "$stage/extracted"
verify_bundle "$stage/extracted/$app"
helper="$stage/extracted/$app/Contents/MacOS/tss-performer"
printf '%s\n' '{"bind":"127.0.0.1","host":"127.0.0.1","port":0}' join hold release |
  "$helper" --verify-loopback > "$stage/performer.log"
for step in joined held released; do
  grep -Fqx "{\"kind\":\"proof-step\",\"name\":\"$step\"}" "$stage/performer.log" ||
    fail "The packaged controller did not complete $step."
done
codesign -d --verbose=2 "$stage/extracted/$app"
mv "$stage/extracted/$app" "$out/$app"
mv "$stage/$zip" "$out/$zip"
shasum -a 256 "$out/$zip"
