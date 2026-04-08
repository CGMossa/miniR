+++
title = "miniR"
sort_by = "weight"
+++

miniR is an R interpreter written in Rust. It is tested against a checked-in `cran/` corpus of real packages, not against toy scripts and not against the full CRAN archive.

## Goals

1. **Run real package code** - 131/260 packages in the checked-in compatibility corpus currently load
2. **Well-written code** - clean, idiomatic Rust with optional subsystems behind feature flags
3. **Reentrant interpreter** - multiple R sessions in one process, with per-interpreter state

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

- **800+ builtin entry points** - base R, stats, utils, and more
- **Lazy evaluation** - R promise semantics for function arguments
- **S3 dispatch** - method dispatch for operators and generics
- **Native code** - `.Call`, `.External`, `.C`, `dyn.load`, and symbolized native backtraces
- **Package loading** - `library()`, `require()`, namespace management, and method registration
- **Grid and graphics devices** - viewports, grobs, SVG, raster, and PDF backends
- **Feature-gated subsystems** - REPL, linalg, TLS, parquet, GUI, and native runtime support
- **Per-interpreter state** - RNG, temp dirs, env vars, working directory, options, and tracebacks
