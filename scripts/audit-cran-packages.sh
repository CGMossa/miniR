#!/bin/bash
# audit-cran-packages.sh — Generate cran-packages.toml from the CRAN corpus
#
# Analyzes every package in cran/ and produces a structured TOML file with:
#   - Package metadata (version, dependencies)
#   - Native code info (C/C++/Fortran file counts)
#   - Build characteristics (bundled libs, system deps, Rcpp, configure)
#   - R C API surface (headers, Rf_* functions, R_* globals, macros)
#   - Compilation and load status
#
# Usage:
#   ./scripts/audit-cran-packages.sh [cran_dir] [output_file]
#
# Defaults:
#   cran_dir   = cran/
#   output_file = cran-packages.toml

set -euo pipefail

CRAN_DIR="${1:-cran}"
OUTPUT="${2:-cran-packages.toml}"
MINIR_INCLUDE="${MINIR_INCLUDE:-include}"
MINIR_BIN="${MINIR_BIN:-target/release/r}"

# ── Phase 1: Gather package metadata ──

echo "Phase 1: Scanning $CRAN_DIR for package metadata..." >&2

METADATA_FILE=$(mktemp)
for pkg in "$CRAN_DIR"/*/; do
  name=$(basename "$pkg")
  desc="$pkg/DESCRIPTION"
  ns="$pkg/NAMESPACE"
  src="$pkg/src"

  # Version
  version=$(grep "^Version:" "$desc" 2>/dev/null | sed 's/Version: *//' || echo "")

  # Source file counts
  has_src="false"
  c_count=0; cpp_count=0; f_count=0
  if [ -d "$src" ]; then
    has_src="true"
    c_count=$(find "$src" -name "*.c" 2>/dev/null | wc -l | tr -d ' ')
    cpp_count=$(find "$src" \( -name "*.cpp" -o -name "*.cc" -o -name "*.cxx" \) 2>/dev/null | wc -l | tr -d ' ')
    f_count=$(find "$src" \( -name "*.f" -o -name "*.f90" -o -name "*.f95" \) 2>/dev/null | wc -l | tr -d ' ')
  fi

  # Package characteristics
  has_dynlib=$(grep -q "useDynLib" "$ns" 2>/dev/null && echo "true" || echo "false")
  has_inst_include=$([ -d "$pkg/inst/include" ] && echo "true" || echo "false")
  has_configure=$([ -f "$pkg/configure" ] && echo "true" || echo "false")

  has_bundled="false"
  if [ -f "$src/Makevars" ] && grep -qE "STATLIB|\.a\b" "$src/Makevars" 2>/dev/null; then
    has_bundled="true"
  fi

  has_syslib="false"
  if [ -d "$src" ] && grep -qr "sodium\|uv\.h\|nlopt\|curl/\|openssl\|png\.h\|config\.h" "$src" 2>/dev/null; then
    has_syslib="true"
  fi

  uses_rcpp="false"
  if [ -d "$src" ] && find "$src" -name "*.c" -o -name "*.cpp" -o -name "*.h" 2>/dev/null | \
     xargs grep -ql '#include.*Rcpp' 2>/dev/null | head -1 | grep -q .; then
    uses_rcpp="true"
  fi

  # Dependencies
  imports=$(grep "^Imports:" "$desc" 2>/dev/null | sed 's/Imports: *//' | tr -d '\n' | head -c 200 || echo "")
  depends=$(grep "^Depends:" "$desc" 2>/dev/null | sed 's/Depends: *//' | tr -d '\n' | head -c 200 || echo "")
  linking_to=$(grep "^LinkingTo:" "$desc" 2>/dev/null | sed 's/LinkingTo: *//' | tr -d '\n' | head -c 200 || echo "")

  # Compiled?
  ext=$(if [ "$(uname)" = "Darwin" ]; then echo dylib; else echo so; fi)
  compiled=$([ -f "$pkg/libs/$name.$ext" ] && echo "true" || echo "false")

  echo "$name|$version|$has_src|$c_count|$cpp_count|$f_count|$has_dynlib|$has_bundled|$has_syslib|$uses_rcpp|$has_configure|$has_inst_include|$compiled|$imports|$depends|$linking_to"
done > "$METADATA_FILE" 2>/dev/null

echo "  Found $(wc -l < "$METADATA_FILE" | tr -d ' ') packages" >&2

# ── Phase 2: Extract R C API usage ──

echo "Phase 2: Extracting R C API usage from native code..." >&2

API_FILE=$(mktemp)
for pkg in "$CRAN_DIR"/*/; do
  name=$(basename "$pkg")
  src="$pkg/src"
  [ -d "$src" ] || continue

  # Headers included
  headers=$(find "$src" \( -name "*.c" -o -name "*.cpp" -o -name "*.h" \) 2>/dev/null | \
    xargs grep -oh '#include <[^>]*>' 2>/dev/null | \
    sort -u | sed 's/#include <//;s/>//' | tr '\n' ',' | sed 's/,$//')

  # Rf_* functions
  rf_funcs=$(find "$src" \( -name "*.c" -o -name "*.cpp" -o -name "*.h" \) 2>/dev/null | \
    xargs grep -oh '\bRf_[a-zA-Z_]*\b' 2>/dev/null | \
    sort -u | tr '\n' ',' | sed 's/,$//')

  # R_* globals and functions (filter out package-internal R_ prefixes)
  r_globals=$(find "$src" \( -name "*.c" -o -name "*.cpp" -o -name "*.h" \) 2>/dev/null | \
    xargs grep -oh '\bR_[A-Z][a-zA-Z_]*\b' 2>/dev/null | \
    sort -u | \
    grep -v 'R_SEXP_to\|R_CHECK\|R_RETURN\|R_NONE\|R_BREAK\|R_CONTINUE\|R_BOUND\|R_META\|R_SIZE\|R_RW\|R_GUARD\|R_BYTES\|R_H_\|R_PRId' | \
    tr '\n' ',' | sed 's/,$//')

  # R API macros
  macros=$(find "$src" \( -name "*.c" -o -name "*.cpp" -o -name "*.h" \) 2>/dev/null | \
    xargs grep -oh '\bPROTECT\b\|\bUNPROTECT\b\|\bSTRING_PTR_RO\b\|\bALTREP\b\|\bMARK_NOT_MUTABLE\b\|\bSET_ATTRIB\b\|\bATTRIB\b\|\bGET_SLOT\b\|\bMAKE_CLASS\b\|\bNEW_OBJECT\b\|\bIS_S4_OBJECT\b\|\bDATAPTR\b\|\bNAMED\b\|\bSET_NAMED\b' 2>/dev/null | \
    sort -u | tr '\n' ',' | sed 's/,$//')

  [ -z "$headers" ] && [ -z "$rf_funcs" ] && [ -z "$r_globals" ] && continue
  echo "$name|$headers|$rf_funcs|$r_globals|$macros"
done > "$API_FILE" 2>/dev/null

echo "  Found API data for $(wc -l < "$API_FILE" | tr -d ' ') packages" >&2

# ── Phase 3: Test load status (optional — slow) ──

LOAD_FILE=$(mktemp)
if [ -x "$MINIR_BIN" ] && [ "${SKIP_LOAD_TEST:-}" != "1" ]; then
  echo "Phase 3: Testing load status (set SKIP_LOAD_TEST=1 to skip)..." >&2
  ext=$(if [ "$(uname)" = "Darwin" ]; then echo dylib; else echo so; fi)
  for pkg in "$CRAN_DIR"/*/; do
    name=$(basename "$pkg")
    [ -f "$pkg/libs/$name.$ext" ] || continue
    result=$(MINIR_INCLUDE="$MINIR_INCLUDE" "$MINIR_BIN" -e "
Sys.setenv(R_LIBS = '$(cd "$CRAN_DIR/.." && pwd)/cran')
tryCatch({library($name); cat('ok')}, error=function(e) cat('err'))
" 2>&1 | grep "^ok\|^err" | head -1)
    echo "$name:${result:-unknown}"
  done > "$LOAD_FILE" 2>/dev/null
  echo "  Tested $(wc -l < "$LOAD_FILE" | tr -d ' ') packages" >&2
else
  echo "Phase 3: Skipped (no miniR binary or SKIP_LOAD_TEST=1)" >&2
fi

# ── Phase 4: Generate TOML ──

echo "Phase 4: Generating $OUTPUT..." >&2

python3 << PYEOF
import re, sys

# Read metadata
metadata = {}
with open('$METADATA_FILE') as f:
    for line in f:
        parts = line.strip().split('|')
        if len(parts) >= 16:
            metadata[parts[0]] = {
                'version': parts[1],
                'has_src': parts[2] == 'true',
                'c': int(parts[3]), 'cpp': int(parts[4]), 'f': int(parts[5]),
                'dynlib': parts[6] == 'true',
                'bundled': parts[7] == 'true',
                'syslib': parts[8] == 'true',
                'rcpp': parts[9] == 'true',
                'configure': parts[10] == 'true',
                'inst_include': parts[11] == 'true',
                'compiled': parts[12] == 'true',
                'imports': parts[13], 'depends': parts[14], 'linking_to': parts[15],
            }

# Read API usage
api = {}
with open('$API_FILE') as f:
    for line in f:
        parts = line.strip().split('|')
        if len(parts) >= 4:
            api[parts[0]] = {
                'headers': [h.strip() for h in parts[1].split(',') if h.strip()],
                'rf': [s.strip() for s in parts[2].split(',') if s.strip()],
                'r_globals': [s.strip() for s in parts[3].split(',') if s.strip()],
                'macros': [s.strip() for s in parts[4].split(',') if s.strip()] if len(parts) > 4 else [],
            }

# Read load status
load_status = {}
with open('$LOAD_FILE') as f:
    for line in f:
        if ':' in line:
            name, status = line.strip().split(':', 1)
            load_status[name] = status

# Generate TOML
out = []
out.append('# CRAN Package Registry for miniR')
out.append(f'# Generated from {len(metadata)} packages in $CRAN_DIR/')
out.append('#')
out.append('# Key fields:')
out.append('#   native_code       — ["c", "cpp", "fortran"] source types')
out.append('#   source_files      — { c = N, cpp = N, fortran = N }')
out.append('#   uses_dynlib       — NAMESPACE has useDynLib directive')
out.append('#   bundled_library   — Makevars builds a static .a before linking')
out.append('#   needs_system_library — requires external system libraries')
out.append('#   uses_rcpp         — includes Rcpp.h (C++ R bridge)')
out.append('#   has_configure     — has a configure script (autoconf)')
out.append('#   provides_headers  — inst/include/ with headers for LinkingTo')
out.append('#   compiles          — native code compiles with miniR')
out.append('#   loads             — library() succeeds in miniR')
out.append('#   blocker           — what prevents loading')
out.append('#   r_headers         — C/C++ headers included from R')
out.append('#   r_api_functions   — Rf_* functions used')
out.append('#   r_api_globals     — R_* global symbols used')
out.append('#   r_api_macros      — R API macros used (PROTECT, ALTREP, etc.)')
out.append('')

for name in sorted(metadata.keys(), key=str.lower):
    m = metadata[name]
    out.append(f'[packages.{name}]')
    out.append(f'version = "{m["version"]}"')

    if m['has_src']:
        types = []
        if m['c'] > 0: types.append('"c"')
        if m['cpp'] > 0: types.append('"cpp"')
        if m['f'] > 0: types.append('"fortran"')
        if types:
            out.append(f'native_code = [{", ".join(types)}]')
            out.append(f'source_files = {{ c = {m["c"]}, cpp = {m["cpp"]}, fortran = {m["f"]} }}')
    else:
        out.append('native_code = []')

    if m['dynlib']: out.append('uses_dynlib = true')
    if m['bundled']: out.append('bundled_library = true')
    if m['syslib']: out.append('needs_system_library = true')
    if m['rcpp']: out.append('uses_rcpp = true')
    if m['configure']: out.append('has_configure = true')
    if m['inst_include']: out.append('provides_headers = true')
    if m['compiled']: out.append('compiles = true')

    if name in load_status:
        out.append(f'loads = {"true" if load_status[name] == "ok" else "false"}')
    elif not m['has_src']:
        out.append('loads = true  # pure R package')

    if m['imports']: out.append(f'imports = "{m["imports"]}"')
    if m['depends']: out.append(f'depends = "{m["depends"]}"')
    if m['linking_to']: out.append(f'linking_to = "{m["linking_to"]}"')

    if name in api:
        a = api[name]
        if a['headers']:
            h = ', '.join(f'"{h}"' for h in sorted(a['headers'])[:20])
            out.append(f'r_headers = [{h}]')
        if a['rf']:
            s = ', '.join(f'"{s}"' for s in sorted(a['rf'])[:30])
            out.append(f'r_api_functions = [{s}]')
        if a['r_globals']:
            g = ', '.join(f'"{g}"' for g in sorted(a['r_globals'])[:20])
            out.append(f'r_api_globals = [{g}]')
        if a['macros']:
            m2 = ', '.join(f'"{m}"' for m in sorted(a['macros']))
            out.append(f'r_api_macros = [{m2}]')

    out.append('')

with open('$OUTPUT', 'w') as f:
    f.write('\n'.join(out))

print(f'  Written {len(metadata)} packages to $OUTPUT', file=sys.stderr)
PYEOF

# Cleanup
rm -f "$METADATA_FILE" "$API_FILE" "$LOAD_FILE"

echo "Done: $OUTPUT" >&2
