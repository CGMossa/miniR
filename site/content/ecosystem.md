+++
title = "Feature Flags And Build Profiles"
weight = 5
description = "What miniR's optional features do, how build profiles compose them, and how the project stays usable for minimal and WASM-oriented builds"
+++

miniR has a relatively small always-on core and a long list of optional subsystems. The feature flags are not only about smaller binaries. They keep parser and evaluator work fast while letting native code, graphics, linalg, TLS, parquet, and GUI support grow without becoming mandatory for every build.

This is also how miniR keeps a path open for embedded and WASM-oriented builds. The dependency shape is designed so the interpreter can shed heavy host-specific subsystems when the target cannot support them.

## Core Crates

These are the main pieces of the always-on Rust stack:

| Area | Crates | Why they are here |
|------|--------|-------------------|
| Parsing | `pest`, `pest_derive` | R grammar and AST construction start here. |
| Builtin registration | `minir-macros`, `linkme` | Attribute macros declare builtins; `linkme` turns them into one registry. |
| Runtime/value plumbing | `derive_more`, `smallvec`, `indexmap`, `itertools` | Reduces boilerplate in value types and call machinery. |
| Strings and matching | `regex`, `aho-corasick`, `memchr`, `unicode-width`, `unicode-segmentation`, `unicase` | Powers string builtins, formatting, and CLI output. |
| Numeric support | `libm`, `num-complex` | Mathematical functions and full complex-number support. |
| Columnar backend | `arrow-array`, `arrow-buffer` | Efficient array-backed storage for vectors and Arrow/Parquet-adjacent work. |
| General utility | `glob`, `temp-dir`, `base64`, `bstr`, `log`, `tracing` | Filesystem helpers, temporary state, binary/text helpers, and instrumentation. |

## Feature-Gated Subsystems

The optional features are where most crate coupling lives:

| Feature | Main crates | What it enables |
|--------|-------------|-----------------|
| `repl` | `reedline`, `crossterm`, `nu-ansi-term` | Interactive line editing, completion, terminal UX |
| `random` | `rand`, `rand_chacha`, `rand_distr` | RNG state, deterministic mode, statistical distributions |
| `datetime` | `jiff` | Time zones, POSIXct/POSIXlt, date formatting |
| `io` | `csv` | Delimited text IO |
| `json` | `serde_json` | JSON parsing and emission |
| `toml` | `toml_edit` | TOML parsing and writing |
| `linalg` | `ndarray`, `nalgebra` | Matrix factorization and linear algebra helpers |
| `tls` | `rustls`, `rustls-native-certs`, `webpki-roots` | HTTPS and secure URL connections |
| `compression` | `flate2`, `bzip2` | Compressed file and serialization support |
| `progress` | `indicatif` | Text progress bars |
| `parallel` | `rayon` | Parallel helpers where the runtime allows them |
| `parquet` | `parquet`, `arrow` | Arrow tables and Parquet files |
| `svg-device` | `svg` | SVG graphics output |
| `raster-device` | `resvg`, `tiny-skia`, `usvg`, `image` | Raster graphics from SVG scene data |
| `pdf-device` | `krilla`, `krilla-svg`, `usvg` | PDF graphics device |
| `plot` / `gui` | `eframe`, `egui`, `egui_plot`, `egui_table`, `winit`, `rfd` | Interactive plotting and GUI viewers |
| `native` | `libloading`, `cc`, `pkg-config`, `addr2line`, `gimli`, `object` | Package compilation, shared-library loading, and native stack unwinding |

## What The Profiles Mean In Practice

The profiles are the fastest way to read the feature graph as an actual development workflow:

- `minimal` means parser and evaluator work with zero optional dependencies.
- `fast` adds common runtime helpers without pulling in the heaviest features.
- `default` is the daily build with the most useful subsystems for ordinary interpreter work.
- `full` is the CI and release-style build where everything additive is turned on.

## Why Some Dependencies Are Coupled

Several feature flags intentionally pull in bundles rather than single crates:

- `native` is not only `libloading`. miniR also needs `cc` and `pkg-config` to build packages, plus `addr2line`, `gimli`, and `object` to turn raw instruction pointers into useful native backtraces.
- `raster-device` and `pdf-device` build on the SVG scene pipeline. That is why they depend on `svg-device` instead of duplicating graphics generation.
- `gui` is layered on top of plotting and viewing support rather than being a separate rendering stack.
- `random` uses both fast and deterministic RNG backends because miniR wants good interactive performance **and** reproducible cross-platform test behavior.

## Build Profiles

The repo exposes a few useful build shapes:

| Profile | Command | Intent |
|--------|---------|--------|
| `minimal` | `cargo build --no-default-features --features minimal` | Parser and evaluator work with the lightest dependency set |
| `fast` | `cargo build --no-default-features --features fast` | Quick iteration with common runtime helpers |
| `default` | `cargo build` | Daily development build |
| `full` | `cargo build --features full` | CI and release-style build with heavy optional subsystems |

## Why This Matters For WASM And Embedded Targets

The project does not want every build to drag along:

- a terminal UI
- native package compilation
- TLS stacks
- GUI plotting
- large linear algebra dependencies

That is why the feature graph exists at all. The minimal end of the project should remain small enough for sandboxed or non-native targets, even when the full interpreter grows significantly.

## Practical Reading Of The Feature Graph

If you are changing parser behavior, basic evaluation, environments, or pure builtins, stay on `minimal` or `fast` as long as possible.

If you are changing:

- package loading or compiled extensions, you probably need `native`
- URL connections, you need `tls`
- matrix-heavy stats functionality, you need `linalg`
- plotting or device output, you need `svg-device`, `raster-device`, `pdf-device`, or `gui`
- wasm-target work, you should start from `minimal` and then add back only what the target can actually support

That split is deliberate. miniR is trying to stay pleasant to work on even while it grows into a serious package runtime.
