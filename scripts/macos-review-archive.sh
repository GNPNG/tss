# Archive a signed app and its adjacent example projects in an empty staging folder.
# Usage from package-macos.sh or test-macos-review-archive.sh.
macos_review_archive() {
  local built=$1 example_dir=$2 stage=$3 archive=$4 pack_dir=${5:-}
  local name pack has_examples=0
  name=$(basename "$built")
  mkdir "$stage/payload" || return 1
  ditto "$built" "$stage/payload/$name" || return 1
  if [[ -d "$example_dir" && -n $(find "$example_dir" -type f -name '*.tss' -print -quit) ]]; then
    has_examples=1
    ditto "$example_dir" "$stage/payload/examples" || return 1
  fi
  if [[ -n "$pack_dir" && -d "$pack_dir" ]]; then
    for pack in "$pack_dir"/*.tsslegends.json; do
      [[ -f "$pack" && ! -L "$pack" ]] || continue
      mkdir -p "$stage/payload/legend-packs" || return 1
      cp "$pack" "$stage/payload/legend-packs/" || return 1
    done
  fi
  ditto -c -k --sequesterRsrc "$stage/payload" "$archive" || return 1
  ditto -x -k "$archive" "$stage/extracted" || return 1
  diff -qr "$built" "$stage/extracted/$name" || return 1
  if [[ -d "$stage/payload/legend-packs" ]]; then
    diff -qr "$stage/payload/legend-packs" "$stage/extracted/legend-packs" || return 1
  fi
  if [[ $has_examples == 1 ]]; then
    diff -qr "$example_dir" "$stage/extracted/examples" || return 1
  else
    [[ ! -e "$stage/extracted/examples" ]] || return 1
  fi
}
