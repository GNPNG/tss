#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 PATH_TO_UPSTREAM_CHECKOUT" >&2
    exit 2
fi

study_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
build_dir=$(mktemp -d "${TMPDIR:-/tmp}/taichi-order.XXXXXXXX")
trap 'rm -rf "$build_dir"' EXIT
result=0

for variant in lbm_solver_3d_2phase lbm_solver_3d_2phase_sparse; do
    source_file="$1/2phase/$variant.py"
    echo "$variant"
    # Isolate the scalar expression that streaming3 writes at the current cell.
    awk '
        /^def streaming3\(\):/ { in_stream = 1; next }
        /^[^[:space:]#]/ { in_stream = 0 }
        in_stream && /^[[:space:]]*psi\[i,j,k\][[:space:]]*=/ {
            sub(/^[^=]*=[[:space:]]*/, "")
            gsub(/rho_r\[i,j,k\]/, "rho_r")
            gsub(/rho_b\[i,j,k\]/, "rho_b")
            print
            matches++
        }
        END { if (matches != 1) exit 1 }
    ' "$source_file" > "$build_dir/expression.rs"
    TAICHI_PHASE_EXPR="$build_dir/expression.rs" rustc --edition 2021 --test \
        "$study_dir/order_parameter_tests.rs" -o "$build_dir/phase-test"
    "$build_dir/phase-test" || result=1
done
exit "$result"
