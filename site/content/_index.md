+++
title = "miniR"
sort_by = "weight"
+++

miniR is an R interpreter written in Rust. It runs real-world R code from popular CRAN packages — not just toy examples.

## Goals

1. **Run top CRAN packages** — 157/260 tested packages load successfully (60%)
2. **Well-written code** — clean, idiomatic Rust; 87K+ lines
3. **Reentrant interpreter** — multiple R sessions in one process, no global state

## Quick Start

```bash
# Clone and build
git clone https://github.com/CGMossa/miniR.git
cd miniR
cargo build --release

# Run R code
./target/release/r -e 'cat("hello from miniR\n")'

# Interactive REPL
./target/release/r
```

## Features

- **800+ built-in functions** — base R, stats, utils, and more
- **Lazy evaluation** — R promise semantics for function arguments
- **S3 dispatch** — method dispatch for all operators and generics
- **Native code** — `.Call`, `.External`, `.C`, `dyn.load` with automatic C/C++/Fortran compilation
- **Package loading** — `library()`, `require()`, namespace management, S3 method registration
- **Grid graphics** — units, viewports, grobs, display list, gpar
- **Interactive plotting** — egui window with SVG/PDF/PNG export
- **Arrow backend** — Float64Array/Int64Array for Double/Integer vectors
