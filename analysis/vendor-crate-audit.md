# Vendor Crate Audit

Audit of all vendored Rust crate dependencies for R interpreter integration potential.

Last updated: 2026-03-09
Crate count: 125

## R-Relevant Crates (have plans/)

### Already integrated

| Crate | Version | Plan | Status |
| ----- | ------- | ---- | ------ |
| csv | 1.4 | plans/csv.md | Integrated — read.csv/write.csv |
| ndarray | 0.17 | plans/ndarray.md | Integrated — matrix multiply, crossprod |
| regex | 1.12 | (no separate plan) | Integrated — grep/grepl/sub/gsub/regexpr |
| reedline | 0.46 | plans/reedline.md | Integrated — REPL |
| pest | 2.8 | (no separate plan) | Integrated — PEG parser |
| tabled | 0.18 | (no separate plan) | Added — not yet used for display |
| derive_more | 2.1 | (no separate plan) | Integrated — Deref/From on newtypes |
| linkme | 0.3 | (no separate plan) | Integrated — distributed builtin registration |
| serde | 1.0 | plans/serde.md | Vendored — not yet used |

### High priority — enable missing R features

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| chrono | 0.4 | plans/chrono.md | Date/time: Date, POSIXct, Sys.time, as.Date, difftime, strptime |
| signal-hook | 0.3 | plans/signal-hook.md | Ctrl+C interrupt handling for long computations |

### Medium priority — improve quality

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| nu-ansi-term | 0.50 | plans/nu-ansi-term.md | Colored error messages, warnings, REPL output |
| thiserror | 2.0 | plans/thiserror.md | Structured RError with derive, "did you mean" suggestions |
| unicode-segmentation | 1.12 | plans/unicode-segmentation.md | Correct nchar() for grapheme clusters (emoji, combining marks) |
| unicode-width | 0.2 | plans/unicode-width.md | Display width for CJK in data frame/matrix printing |

### Low priority — nice to have

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| sha2 | 0.10 | plans/sha2.md | Hashing/checksums (digest, md5sum) |
| unicase | 2.9 | plans/unicase.md | Unicode case-insensitive string comparison |
| memchr | 2.8 | plans/memchr.md | SIMD-accelerated fixed=TRUE grep/grepl |
| fnv | 1.0 | plans/fnv.md | Faster environment HashMap lookups |
| smallvec | 1.15 | plans/smallvec.md | Stack-allocated small vectors for attrs/args |

### Have plans but not yet vendored

| Crate | Plan | R feature |
| ----- | ---- | --------- |
| polars | plans/polars-dataframe.md | Data frame backend |
| egui_table + eframe | plans/egui-table-view.md | View() spreadsheet display |
| rand | plans/rand.md | rnorm, runif, rbinom, set.seed |
| jiff | plans/jiff.md | Alternative to chrono for date/time |
| rayon | plans/rayon.md | Parallel computation |
| indexmap | plans/indexmap.md | Ordered hash maps |
| walkdir | plans/walkdir.md | list.files(recursive=TRUE) |
| dirs | plans/dirs.md | path.expand("~"), Sys.getenv("HOME") |
| globset | plans/globset.md | Sys.glob() |
| miette | plans/miette.md | Fancy error diagnostics |
| ctrlc | plans/ctrlc.md | Alternative to signal-hook for Ctrl+C |

## Infrastructure Crates (no R relevance)

These are build tools, proc-macro plumbing, platform bindings, and transitive deps. No plan needed.

### Build / compilation

autocfg, cc, find-msvc-tools, rustc_version, rustversion, semver, shlex, version_check

### Proc-macro plumbing

convert_case, document-features, heck, linkme-impl, litrs, proc-macro-error-attr2, proc-macro-error2, proc-macro2, quote, strum, strum_macros, syn, derive_more-impl, tabled_derive, thiserror-impl, serde_derive, serde_core

### Platform bindings

android_system_properties, core-foundation-sys, errno, libc, linux-raw-sys, redox_syscall, rustix, winapi, winapi-i686-pc-windows-gnu, winapi-x86_64-pc-windows-gnu, windows-core, windows-implement, windows-interface, windows-link, windows-result, windows-strings, windows-sys, windows-sys-0.59.0, windows-targets, windows_aarch64_gnullvm, windows_aarch64_msvc, windows_i686_gnu, windows_i686_gnullvm, windows_i686_msvc, windows_x86_64_gnu, windows_x86_64_gnullvm, windows_x86_64_msvc, crossterm_winapi

### Crypto internals (transitive deps of sha2)

block-buffer, cpufeatures, crypto-common, digest, generic-array, typenum

### Parser internals (transitive deps of pest)

pest_derive, pest_generator, pest_meta, ucd-trie

### Regex internals

regex-automata, regex-syntax

### Concurrency primitives

lock_api, once_cell, parking_lot, parking_lot_core, portable-atomic, portable-atomic-util, scopeguard

### I/O and terminal

crossterm, fd-lock, mio, signal-hook-mio, signal-hook-registry, strip-ansi-escapes, vte

### Numeric (transitive deps of ndarray)

matrixmultiply, num-integer, num-traits, rawpointer

### String / text utilities

aho-corasick, bytecount, itoa, ryu, unicode-ident

### Data structures

bitflags, bumpalo, either, smallvec

### Time (transitive dep of chrono)

iana-time-zone, iana-time-zone-haiku

### Serialization

csv-core, serde, serde_core, serde_derive

### Display

nu-ansi-term, papergrid

### WASM (transitive deps, not relevant)

js-sys, wasi, wasm-bindgen, wasm-bindgen-macro, wasm-bindgen-macro-support, wasm-bindgen-shared

### Misc

cfg-if, either, fnv, log, memchr, num-complex, sha2, unicase
