#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/macos-review-archive.sh"
trial=$(mktemp -d)
trap 'rm -rf "$trial"' EXIT
app="$trial/Test.app"
mkdir -p "$app/Contents"
printf '%s\n' 'immutable bundle data' > "$app/Contents/payload"

for fixture in absent empty projects; do
  stage="$trial/$fixture"
  mkdir "$stage"
  case "$fixture" in
    empty) mkdir "$stage/examples" ;;
    projects)
      mkdir -p "$stage/examples/first"
      printf '%s\n' '{}' > "$stage/examples/first/Sample.tss"
      printf '%s\n' 'companion media' > "$stage/examples/first/Motif clip.mp4" ;;
  esac
  macos_review_archive "$app" "$stage/examples" "$stage" "$stage/App.zip"
  cmp "$app/Contents/payload" "$stage/extracted/Test.app/Contents/payload"
  if [[ "$fixture" == projects ]]; then
    cmp "$stage/examples/first/Motif clip.mp4" "$stage/extracted/examples/first/Motif clip.mp4"
    cmp "$stage/examples/first/Sample.tss" "$stage/extracted/examples/first/Sample.tss"
  else
    test ! -e "$stage/extracted/examples"
  fi
done
echo "Verified review archives with absent, empty and populated example directories."
