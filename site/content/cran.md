+++
title = "CRAN Corpus Compatibility"
weight = 9
description = "Compatibility numbers for the checked-in `cran/` corpus, not the full CRAN archive"
+++

The compatibility numbers on this site refer to the checked-in `cran/` tree in the repository. They do **not** refer to the full CRAN archive.

The local corpus is refreshed with `just update-cran-test-packages` and is meant to stress the interpreter with real package code: base packages, recommended packages, and a large set of popular CRAN packages.

## Current Headline

- Latest repo scan: **131 / 260** packages in the checked-in corpus load successfully.
- Parse-only corpus checks are separate and opt in via `MINIR_PARSE_CRAN=1 cargo test --test parse_corpus`.
- The remaining compatibility work is mostly runtime, package loading, object system, and native-code work - not parser work.

## What The Corpus Contains

- Checked-in package trees under `cran/`, so corpus runs are reproducible from the repo state.
- Base and recommended packages, not only third-party CRAN packages.
- A compatibility denominator tied to the repo snapshot, which is why miniR talks about a corpus rather than "all of CRAN".
- Real package assets such as `R/`, `NAMESPACE`, `man/`, `src/`, and `inst/`, not just parser fixtures.

## Representative Packages Already Loading

Examples from the current corpus that load:

- Tidyverse core pieces such as `rlang`, `vctrs`, `tibble`, `dplyr`, `purrr`, `forcats`, and `tidyselect`
- Web and reporting packages such as `knitr`, `rmarkdown`, `htmltools`, `htmlwidgets`, and `bslib`
- Time and statistics packages such as `lubridate`, `timechange`, `lmtest`, and `sandwich`
- IO and systems packages such as `xml2`, `curl`, `jsonlite`, `yaml`, and `readxl`

## What Blocks More Packages

| Area | Why it blocks the corpus |
|------|---------------------------|
| Package runtime | The corpus expects namespaces, imports, exports, hooks, datasets, and base/recommended package environments to behave like packages, not loose files. |
| Native code | Many packages ship `src/` trees and expect `.Call`, `.External`, routine registration, and package build/link behavior. |
| Object systems | S3 already matters, and packages also lean on `methods`, S4, and newer class machinery. |
| Data-model fidelity | Attributes, `data.frame`, factors, recycling, subsetting, and replacement semantics still decide whether package code survives contact with reality. |
| Graphics and devices | The corpus assumes `graphics`, `grDevices`, and `grid` exist as runtime subsystems, not optional niceties. |

## System Libraries Still Matter

The corpus includes many native packages that benefit from system libraries discovered through `pkg-config`, for example `xml2`, `fs`, `sass`, `openssl`, and packages layered on top of them.
