# https://just.systems

root := justfile_directory()

default:
    echo 'Hello, world!'

# Run all lints: cargo fmt check, clippy, markdownlint
lint:
    cargo fmt --check
    cargo clippy -- -D warnings
    npx --yes markdownlint-cli2 '**/*.md' '#vendor' '#cran' '#target'

# Re-vendor crates, skipping if Cargo.toml/Cargo.lock files haven't changed
vendor:
    #!/usr/bin/env bash
    set -euo pipefail
    HASH_FILE="{{root}}/vendor/.cargo-lock-hash"
    # Hash all Cargo.toml and Cargo.lock files (excluding vendor/)
    CURRENT_HASH=$(find "{{root}}" -path "{{root}}/vendor" -prune -o \
        -path "{{root}}/cran" -prune -o \
        -path "{{root}}/target" -prune -o \
        \( -name 'Cargo.toml' -o -name 'Cargo.lock' \) -print \
        | sort | xargs cat | md5)
    if [ -f "$HASH_FILE" ] && [ "$(cat "$HASH_FILE")" = "$CURRENT_HASH" ]; then
        echo "vendor/ is up to date (no Cargo.toml/Cargo.lock changes)"
        exit 0
    fi
    cargo vendor "{{root}}/vendor"
    printf '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "{{root}}/vendor"\n' > "{{root}}/.cargo/config.toml"
    printf '# vendor\n\nVendored Rust crate dependencies (managed by `cargo vendor`).\n\nRun `just vendor` to update.\n' > "{{root}}/vendor/README.md"
    echo "$CURRENT_HASH" > "$HASH_FILE"
    just vendor-apply-patches

# Find minimum supported Rust version (requires cargo-msrv)
find-msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo msrv --version &>/dev/null; then
        echo "error: cargo-msrv is not installed" >&2
        echo "  install it with: cargo install cargo-msrv" >&2
        exit 1
    fi
    cargo msrv find --min 1.66

# Re-vendor crates unconditionally (ignores hash check)
vendor-force:
    #!/usr/bin/env bash
    set -euo pipefail
    HASH_FILE="{{root}}/vendor/.cargo-lock-hash"
    cargo vendor "{{root}}/vendor"
    printf '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "{{root}}/vendor"\n' > "{{root}}/.cargo/config.toml"
    printf '# vendor\n\nVendored Rust crate dependencies (managed by `cargo vendor`).\n\nRun `just vendor` to update.\n' > "{{root}}/vendor/README.md"
    # Update hash after forced vendor
    CURRENT_HASH=$(find "{{root}}" -path "{{root}}/vendor" -prune -o \
        -path "{{root}}/cran" -prune -o \
        -path "{{root}}/target" -prune -o \
        \( -name 'Cargo.toml' -o -name 'Cargo.lock' \) -print \
        | sort | xargs cat | md5)
    echo "$CURRENT_HASH" > "$HASH_FILE"
    just vendor-apply-patches

# Show local modifications to vendored crates (diffs against a fresh cargo vendor)
vendor-diff:
    #!/usr/bin/env bash
    set -euo pipefail
    FRESH=$(mktemp -d)
    trap 'rm -rf "$FRESH"' EXIT
    echo "Vendoring to temp dir for comparison..."
    CARGO_SOURCE_CRATES_IO_REPLACE_WITH="" CARGO_SOURCE_VENDORED_SOURCES_DIRECTORY="" \
        cargo vendor "$FRESH" 2>/dev/null
    diff -rq "{{root}}/vendor/" "$FRESH/" \
        --exclude='.cargo-checksum.json' \
        --exclude='.cargo-lock-hash' \
        --exclude='README.md' \
        --exclude='.gitignore' \
        | grep -v "^Only in $FRESH" || echo "No local modifications found."

# Apply patches from vendor-patches/ to vendored crates
vendor-apply-patches:
    #!/usr/bin/env bash
    set -euo pipefail
    PATCH_ROOT="{{root}}/vendor-patches"
    if [ ! -d "$PATCH_ROOT" ]; then
        exit 0
    fi
    FOUND=false
    for patch_dir in "$PATCH_ROOT"/*/; do
        [ -d "$patch_dir" ] || continue
        crate=$(basename "$patch_dir")
        if [ ! -d "{{root}}/vendor/$crate" ]; then
            echo "warning: vendor-patches/$crate has no matching vendor/$crate"
            continue
        fi
        for patch in "$patch_dir"*.patch; do
            [ -f "$patch" ] || continue
            FOUND=true
            echo "applying $patch to vendor/$crate"
            (cd "{{root}}/vendor/$crate" && patch -p1 < "$patch")
        done
        # recalculate checksums for patched crate
        "{{root}}/scripts/fix-vendor-checksum.sh" "{{root}}/vendor/$crate"
    done
    if [ "$FOUND" = false ]; then
        echo "No vendor patches to apply."
    fi

# Update Cargo.lock from crates.io, bypassing vendor source replacement
update *args:
    CARGO_SOURCE_CRATES_IO_REPLACE_WITH="" CARGO_SOURCE_VENDORED_SOURCES_DIRECTORY="" cargo update {{args}}

# Dump public API of a vendored crate as rustdoc JSON (requires nightly + jq)
crate-docs crate:
    #!/usr/bin/env bash
    set -euo pipefail
    LIB="{{root}}/vendor/{{crate}}/src/lib.rs"
    [ -f "$LIB" ] || { echo "error: $LIB not found" >&2; exit 1; }
    ED=$(grep -oP 'edition\s*=\s*"\K[^"]+' "{{root}}/vendor/{{crate}}/Cargo.toml" 2>/dev/null || echo "2021")
    OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
    rustup run nightly rustdoc --edition "$ED" \
        -Z unstable-options --output-format json --output "$OUT" "$LIB" 2>/dev/null
    JSON=$(find "$OUT" -name '*.json' | head -1)
    [ -n "$JSON" ] || { echo "error: no JSON produced" >&2; exit 1; }
    jq -r '
      .index | to_entries[]
      | select(.value.visibility == "public")
      | select(.value.name != null)
      | (.value.inner | keys[0]) as $kind
      | select($kind != null)
      | "\($kind): \(.value.name)"
    ' "$JSON" | sort -u

# Show lines of code (requires tokei)
loc:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v tokei &>/dev/null; then
        echo "error: tokei is not installed" >&2
        echo "  install it with: cargo install tokei" >&2
        exit 1
    fi
    tokei src/ minir-macros/src/

# Show per-file lines of code breakdown, sorted by lines (requires tokei)
loc-detail:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v tokei &>/dev/null; then
        echo "error: tokei is not installed" >&2
        echo "  install it with: cargo install tokei" >&2
        exit 1
    fi
    tokei -f -e vendor/ -e cran/ -e target/ -s lines

# Find Rust source files over 500 lines in src/
large-files:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Rust files over 500 lines in src/:"
    echo "---"
    find src/ minir-macros/src/ -name '*.rs' -exec wc -l {} + | sort -rn | head -20

# Clone top CRAN packages for compatibility testing
update-cran-test-packages:
    #!/usr/bin/env bash
    set -euo pipefail
    D="{{root}}/cran"; mkdir -p "$D"; FAIL=0; N=0

    grab() { # grab <github-url> <dest> <label>
        if [ -d "$2" ]; then echo "  $3 (skip)"; return 0; fi
        echo -n "  $3... "
        git clone --depth 1 --quiet "$1" "$2" 2>/dev/null && rm -rf "$2/.git" && echo "ok" || { echo "FAIL"; return 1; }
    }

    # R base packages (sparse checkout from r-devel/r-svn)
    echo "=== R base ==="
    BASE=(base compiler datasets grDevices graphics grid methods parallel splines stats stats4 tcltk tools utils)
    if [ ! -d "$D/.r-svn-sparse" ]; then
        echo -n "  Cloning R source (sparse)... "
        git clone --depth 1 --filter=blob:none --sparse --quiet \
            https://github.com/r-devel/r-svn.git "$D/.r-svn-sparse" 2>/dev/null
        cd "$D/.r-svn-sparse"
        git sparse-checkout set $(printf 'src/library/%s ' "${BASE[@]}") 2>/dev/null
        echo "ok"; cd "$D"
    fi
    for p in "${BASE[@]}"; do
        if [ -d "$D/$p" ]; then echo "  $p (skip)"
        elif [ -d "$D/.r-svn-sparse/src/library/$p" ]; then cp -r "$D/.r-svn-sparse/src/library/$p" "$D/$p"; echo "  $p ok"
        else echo "  $p FAIL"; FAIL=$((FAIL+1)); fi
        N=$((N+1))
    done
    rm -rf "$D/.r-svn-sparse"

    # Recommended packages
    echo -e "\n=== Recommended ==="
    REC=(MASS Matrix KernSmooth boot class cluster codetools foreign lattice mgcv nlme nnet rpart spatial survival)
    for p in "${REC[@]}"; do grab "https://github.com/cran/$p.git" "$D/$p" "$p" || FAIL=$((FAIL+1)); N=$((N+1)); done

    # Top 200 CRAN downloads
    echo -e "\n=== Top 200 ==="
    TOP=(
        rlang vctrs ggplot2 lifecycle cli tibble R6 pillar magrittr glue
        Rcpp withr scales cpp11 dplyr isoband utf8 gtable S7 pkgconfig
        generics viridisLite RColorBrewer farver processx jsonlite labeling callr RcppEigen ps
        xfun stringr purrr curl tidyr backports knitr htmltools digest yaml
        tidyselect pkgbuild rmarkdown stringi mime fs tidyverse rappdirs tinytex evaluate
        jquerylib desc crayon httr base64enc sass bslib systemfonts fastmap cachem
        gridExtra fontawesome readxl highr memoise rstan StanHeaders textshaping BH openssl
        readr ragg abind numDeriv hms data.table xml2 askpass sys rstudioapi
        prettyunits checkmate bit64 progress loo bit tzdb zoo posterior distributional
        lubridate rprojroot vroom clipr DBI commonmark RcppParallel matrixStats QuickJSR inline
        remotes shiny rvest lazyeval dbplyr httpuv colorspace uuid selectr googledrive
        xtable waldo diffobj brio crosstalk ids praise reprex conflicted gargle
        sourcetools igraph e1071 rematch2 forecast plyr nloptr lmtest proxy openxlsx
        reshape2 xts png zip timeDate httr2 quantreg DT sf MatrixModels
        pbkrtest arrow minqa cowplot Formula ggrepel units quantmod V8 carData
        s2 mvtnorm TTR SparseM sessioninfo future classInt wk parallelly globals
        janitor gh foreach miniUI patchwork plotly renv assertthat future.apply tseries
        devtools sandwich reticulate ggpubr sp roxygen2 gtools corrplot RSQLite rjson
        writexl terra gert usethis pkgdown Hmisc brew later R.utils timechange
        promises htmlwidgets haven broom whisker lattice viridis blob survival cellranger
    )
    for p in "${TOP[@]}"; do grab "https://github.com/cran/$p.git" "$D/$p" "$p" || FAIL=$((FAIL+1)); N=$((N+1)); done

    # Extra target packages from cran-target-packages.txt
    EXTRAS_FILE="{{root}}/cran-target-packages.txt"
    if [ -f "$EXTRAS_FILE" ]; then
        echo -e "\n=== Target packages ==="
        while IFS= read -r line || [ -n "$line" ]; do
            line="${line%%#*}"   # strip comments
            line="${line// /}"   # strip whitespace
            [ -z "$line" ] && continue
            grab "https://github.com/cran/$line.git" "$D/$line" "$line" || FAIL=$((FAIL+1))
            N=$((N+1))
        done < "$EXTRAS_FILE"
    fi

    echo -e "\nDone. $((N-FAIL))/$N packages ($FAIL failed)."
