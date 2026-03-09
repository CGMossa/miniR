# https://just.systems

root := justfile_directory()

default:
    echo 'Hello, world!'

# Re-vendor crates (cargo vendor resolves from crates.io, bypassing source replacement)
vendor:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo vendor --quiet "{{root}}/vendor"
    printf '# vendor\n\nVendored Rust crate dependencies (managed by `cargo vendor`).\n\nRun `just vendor` to update.\n' > "{{root}}/vendor/README.md"

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

    echo -e "\nDone. $((N-FAIL))/$N packages ($FAIL failed)."
