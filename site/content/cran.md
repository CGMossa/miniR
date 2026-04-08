+++
title = "CRAN Package Compatibility"
weight = 3
description = "157/260 packages load --- 60% of the tested corpus"
+++

miniR loads **157 out of 260** tested CRAN packages (60%).

## Tidyverse Core

All load successfully:

| Package | Status |
|---------|--------|
| rlang | OK |
| lifecycle | OK |
| vctrs | OK |
| tibble | OK |
| dplyr | OK |
| purrr | OK |
| forcats | OK |
| tidyselect | OK |
| tidyverse | OK |

## Notable Packages

| Category | Packages |
|----------|----------|
| **Data wrangling** | dplyr, tibble, tidyr*, readr, dbplyr, broom |
| **Visualization** | plotly, ggpubr, gridExtra, scales |
| **Web/HTML** | knitr, rmarkdown, bslib, htmlwidgets, htmltools, sass |
| **Package dev** | Rcpp, RcppEigen, cli, rlang |
| **Time/Date** | lubridate, timechange, hms |
| **Spatial** | sp, classInt |
| **Financial** | quantmod, TTR, tseries, xts, zoo |
| **Statistics** | lmtest, quadprog, urca, sandwich |
| **I/O** | xml2, curl, jsonlite, yaml, readxl |

*tidyr requires stringi which needs ICU system library

## Remaining Blockers

| Blocker | Packages Affected | Status |
|---------|-------------------|--------|
| **S7 class system** | ggplot2, cowplot, patchwork, viridis | Deep class machinery |
| **rlang on_load hooks** | later, promises, httpuv, shiny | topenv() scoping issue |
| **stringi (ICU)** | stringr, tidyr, reshape2 | Needs configure emulation |
| **Matrix (SuiteSparse)** | survival, mgcv, igraph | Needs bundled lib build |
| **Native segfaults** | data.table, ps, openssl, haven | C API runtime gaps |

## System Dependencies

miniR uses `pkg-config` to find system libraries for native packages:

```bash
brew install openssl libxml2 libuv libsass  # macOS
```

Packages that benefit: xml2, fs, sass, sodium, openssl.
