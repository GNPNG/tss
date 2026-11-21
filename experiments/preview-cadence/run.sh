#!/bin/bash
set -euo pipefail
export LC_ALL=C
study=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$study/../.." && pwd)
cd "$repo"
guard="$repo/experiments/gpu-job-control-rust/target/debug/tss-gpu-job"
reservation=/tmp/tss-radeon-pro-5300m-reservation
baseline=${TSS_CADENCE_BASELINE:-6ca9c32083b88423345c45d4f0ae2eb237cd17fa}
runtime=${TSS_CADENCE_NODE:-node}
mode=${1:-cadence}
case "$mode" in cadence|diagnose) ;; *) exit 2 ;; esac
mkdir -p target/preview-cadence
stage=$(mktemp -d "$repo/target/preview-cadence/run.XXXXXX")
for variant in baseline working; do mkdir -p "$stage/$variant/assets"; done
git archive "$baseline" 'WebAssets/*.js' 'WebAssets/*.css' 'WebAssets/*.html' 'WebAssets/assets/*.json' | tar xf - -C "$stage/baseline" --strip-components 1
cp WebAssets/*.js WebAssets/*.css WebAssets/*.html "$stage/working/"
cp WebAssets/assets/*.json "$stage/working/assets/"
/usr/bin/clang -fobjc-arc -framework AppKit -framework WebKit -Wall -O1 "$study/host.m" -o "$stage/cadence-host"
printf 'Results directory %s\n' "$stage"
if [[ -e "$reservation" ]] || pgrep -f '[/](the-sacred-swirls|The Sacred Swirls|cadence-host|tss-oxide-vt-smoke)( |$)' >/dev/null; then
    echo 'Existing Mac graphics reservation, editor or test session' >&2; exit 2
fi
/usr/sbin/ioreg -r -c IOAccelerator -d 1 -l >"$stage/ioreg-before.txt"
"$guard" run --reservation "$reservation" --timeout 115 --output-log "$stage/host.log" -- \
    "$runtime" "$study/serve.mjs" "$stage" "$stage/cadence-host" "$mode"
/usr/sbin/ioreg -r -c IOAccelerator -d 1 -l >"$stage/ioreg-after.txt"
if [[ "$mode" == cadence ]]; then
    "$runtime" "$study/report.mjs" "$stage" "$baseline"
else
    "$runtime" "$study/diagnostic-report.mjs" "$stage"
fi
