+++
title = "Getting Started"
weight = 1
description = "Installation, build profiles, and first steps"
+++

## Prerequisites

- Rust (latest stable)
- On macOS: Xcode command line tools (`xcode-select --install`)
- Optional: `pkg-config` and system libraries for native R packages

## Building

```bash
git clone https://github.com/CGMossa/miniR.git
cd miniR
cargo build --release
```

### Build Profiles

| Profile | Command | Build Time | Use Case |
|---------|---------|-----------|----------|
| **minimal** | `cargo build --no-default-features -F minimal` | ~3s | Parser work, WASM |
| **fast** | `cargo build --no-default-features -F fast` | ~5s | Quick iteration |
| **default** | `cargo build` | ~8.5s | Everyday development |
| **full** | `cargo build -F full` | ~15s | CI, release builds |

## Running

```bash
# Execute an expression
./target/release/r -e 'print(1:10)'

# Run a script
./target/release/r script.R

# Interactive REPL
./target/release/r
```

## Loading CRAN Packages

Point `R_LIBS` at a directory containing installed R packages:

```bash
# Set the library path
export R_LIBS=/path/to/R/library

# Load a package
./target/release/r -e 'library(dplyr); glimpse(mtcars)'
```

miniR currently loads 131/260 packages in the checked-in compatibility corpus, including the tidyverse core (`rlang`, `dplyr`, `tibble`, `purrr`, `vctrs`, `forcats`).

## System Libraries for Native Packages

Some CRAN packages with C/C++ code need system libraries. miniR uses `pkg-config` to find them:

```bash
# macOS (Homebrew)
brew install openssl libxml2 libuv libsass

# The packages that benefit:
# openssl → httr, covr
# libxml2 → xml2
# libuv → fs → htmlwidgets, rmarkdown, bslib
# libsass → sass → bslib, rmarkdown
```
