#!/bin/bash
# Build and run the native stacktrace test.
# Run from the project root: ./tests/native_stacktrace/run.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMPDIR="${TMPDIR:-/tmp}"
OUT="$TMPDIR/stacktest"

echo "==> Compiling test C code..."
cc -c -fPIC -g -fno-omit-frame-pointer \
    -I "$ROOT/include" -I "$ROOT/include/miniR" \
    -o "$OUT.o" "$ROOT/tests/native_stacktrace/test.c"
cc -shared -g -o "$OUT.dylib" "$OUT.o" -undefined dynamic_lookup

echo "==> Generating dSYM (macOS DWARF)..."
dsymutil "$OUT.dylib" 2>/dev/null || true

echo "==> Running with miniR..."
MINIR_INCLUDE="$ROOT/include" "$ROOT/target/debug/r" -e "
dyn.load('$OUT.dylib')
validate <- function(x) .Call('C_validate', as.integer(x))
run_check <- function(x) validate(x)
run_check(-5)
" 2>&1 || true

echo ""
echo "==> Done. Expected: three [C] frames with file:line info."
