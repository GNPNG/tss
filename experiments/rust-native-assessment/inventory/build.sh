#!/bin/sh
# Resolve both release-target graphs against the app lock. An existing pair of
# per-target Cargo trees can be supplied when target caches reside on two hosts.
set -eu
export LC_ALL=C
study=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$study/../../.." && pwd)
cd "$repo"
work=${TSS_TREE_DIR:-"$repo/target/rust-assessment/inventory"}
mkdir -p "$work"
if [ "${1:-}" != "--existing-trees" ]; then
  for os in mac win; do
    case "$os" in
      mac) target=x86_64-apple-darwin ;;
      win) target=x86_64-pc-windows-msvc ;;
    esac
    for kind in normal all; do
      case "$kind" in
        normal) edges=normal,no-proc-macro ;;
        all) edges=normal,build ;;
      esac
      cargo +1.88.0 tree --locked --offline --manifest-path src-tauri/Cargo.toml \
        --target "$target" --features tauri/custom-protocol --edges "$edges" \
        --prefix depth --format '{p}|{l}' > "$work/$os-$kind.tree"
    done
  done
fi
for file in "$work"/mac-normal.tree "$work"/mac-all.tree "$work"/win-normal.tree "$work"/win-all.tree; do
  head -n 1 "$file" | grep -q '^0the-sacred-swirls v' || { echo "Incomplete tree: $file" >&2; exit 1; }
done
awk -F '|' '
{
  sub(/\r$/,"")
  split(FILENAME,path,"/")
  filename=path[length(path)]
  split(filename,parts,"-")
  os=parts[1]
  spec=$1
  match(spec,/^[0-9]+/)
  depth=substr(spec,1,RLENGTH)+0
  spec=substr(spec,RLENGTH+1)
  split(spec,coord," ")
  key=coord[1] "\t" substr(coord[2],2)
  license=$2
  sub(/ \(\*\)$/,"",license)
  licenses[key]=length(license) ? license : "(undeclared)"
  if (filename ~ /-all\.tree$/) {
    if (depth<=1) immediate[key,os]=1
    if (!((key SUBSEP os) in roles)) roles[key,os]="build/proc-macro"
  } else if (depth==0) roles[key,os]="app"
  else if (depth==1) roles[key,os]="direct"
  else if (roles[key,os]!="direct") roles[key,os]="transitive"
}
END {
  for (key in licenses) {
    m=roles[key,"mac"]; w=roles[key,"win"]
    if (m=="build/proc-macro" && immediate[key,"mac"]) m="direct build"
    if (w=="build/proc-macro" && immediate[key,"win"]) w="direct build"
    print key "\t" licenses[key] "\t" (m ? m : "-") "\t" (w ? w : "-")
  }
}' "$work/mac-normal.tree" "$work/win-normal.tree" "$work/mac-all.tree" "$work/win-all.tree" | sort > "$work/package-rows.tsv"
{ printf 'package\tversion\tdeclared license\tx86_64-apple-darwin\tx86_64-pc-windows-msvc\n'; cat "$work/package-rows.tsv"; } > "$study/packages.tsv"
for module in src-tauri live-wheel; do
  cargo +1.88.0 metadata --no-deps --locked --format-version 1 --manifest-path "$module/Cargo.toml" |
    jq -r '.packages[] | .name as $root | .dependencies[] | [$root, .name,.req,(.kind//"normal"),(.target//"all"),(.uses_default_features|tostring),(.features|if length==0 then "-" else join(",") end)] | @tsv' > "$work/$module.tsv"
done
{ printf 'manifest\tdependency\trequirement\tkind\ttarget\tdefault features\texplicit features\n'; sort "$work/src-tauri.tsv" "$work/live-wheel.tsv"; } > "$study/manifests.tsv"
jq -er '
  ["catalog group","library or reference","language and API","integration decision","first reference"],
  (.[] | [.group,.name,.stack,.decision,.sources[0]]) | @tsv
' docs/library-catalog.json > "$study/catalog.tsv"
