# Vendor Crate Audit

Audit of all vendored Rust crate dependencies for R interpreter integration potential.

Last updated: 2026-03-17
Crate count: 239

## R-Relevant Crates (have plans/)

### Already integrated

| Crate | Version | Plan | Status |
| ----- | ------- | ---- | ------ |
| bstr | 1.12 | plans/bstr.md | Integrated -- byte string support for encoding-aware strings |
| csv | 1.4 | plans/csv.md | Integrated -- read.csv/write.csv |
| derive_more | 2.1 | (no separate plan) | Integrated -- Deref/From on newtypes |
| glob | 0.3 | plans/glob.md | Integrated -- Sys.glob() file pattern matching |
| indexmap | 2 | plans/indexmap.md | Integrated -- ordered hash maps for named lists |
| itertools | 0.14 | plans/itertools.md | Integrated -- iterator utilities for builtins |
| jiff | 0.2 | plans/jiff.md | Integrated -- date/time (Date, POSIXct, Sys.time) |
| libm | 0.2 | plans/libm.md | Integrated -- gamma, lgamma, erf, Bessel functions |
| linkme | 0.3 | (no separate plan) | Integrated -- distributed builtin registration |
| ndarray | 0.17 | plans/ndarray.md | Integrated -- matrix multiply, crossprod |
| nu-ansi-term | 0.50 | plans/nu-ansi-term.md | Integrated -- colored REPL output |
| num-complex | 0.4 | plans/num-complex.md | Integrated -- complex number arithmetic |
| pest | 2.8 | (no separate plan) | Integrated -- PEG parser |
| rand | 0.10 | plans/rand.md | Integrated -- rnorm, runif, rbinom, sample |
| rand_distr | 0.6 | plans/rand.md | Integrated -- statistical distributions |
| reedline | 0.46 | plans/reedline.md | Integrated -- REPL |
| regex | 1 | (no separate plan) | Integrated -- grep/grepl/sub/gsub/regexpr |
| serde_json | 1.0 | plans/serde-json.md | Integrated -- fromJSON/toJSON builtins |
| sha2 | 0.10 | plans/sha2.md | Integrated -- digest module (sha256) |
| signal-hook | 0.4 | plans/signal-hook.md | Integrated -- Ctrl+C interrupt handling |
| tabled | 0.20 | (no separate plan) | Integrated -- table display |
| tabwriter | 1 | plans/tabwriter.md | Integrated -- aligned text output |
| temp-dir | 0.2 | plans/temp-dir.md | Integrated -- tempdir/tempfile builtins |
| toml_edit | 0.22 | plans/toml-edit.md | Integrated -- TOML parsing (read.toml/write.toml) |
| unicode-segmentation | 1 | plans/unicode-segmentation.md | Integrated -- nchar() grapheme clusters |
| unicode-width | 0.2 | plans/unicode-width.md | Integrated -- CJK display width |
| unicase | 2 | plans/unicase.md | Integrated -- case-insensitive string comparison |
| walkdir | 2 | plans/walkdir.md | Integrated -- list.files(recursive=TRUE) |
| dirs | 6 | plans/dirs.md | Integrated -- path.expand("~"), Sys.getenv("HOME") |
| globset | 0.4 | plans/globset.md | Integrated -- glob pattern matching for list.files |
| slotmap | 1.0 | plans/slotmap.md | Integrated -- arena allocator for interpreter state |
| miette | 7 | plans/miette.md | Integrated -- fancy error diagnostics |

### High priority -- enable missing R features

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| chacha20 | 0.10 | plans/chacha20.md | Seedable RNG for set.seed() reproducibility |
| flate2 | 1.1 | plans/flate2.md | gzip/deflate compression for connections and saveRDS (vendored, not yet a dep) |
| rayon | 1 | plans/rayon.md | Parallel computation (parallel feature) |

### Medium priority -- improve quality

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| textwrap | 0.16 | plans/textwrap.md | strwrap(), formatDL(), help text formatting |
| terminal_size | 0.4 | plans/terminal-size.md | getOption("width") from terminal, adaptive display |
| rustls | 0.23 | plans/rustls.md | HTTPS support (url, download.file) -- tls feature |
| serde | 1.0 | plans/serde.md | Serialization infrastructure for data interchange |
| thiserror | 2.0 | plans/thiserror.md | Structured RError with derive (superseded by derive_more error) |

### Low priority -- nice to have

| Crate | Version | Plan | R feature |
| ----- | ------- | ---- | --------- |
| memchr | 2.8 | plans/memchr.md | SIMD-accelerated fixed=TRUE grep/grepl |
| fnv | 1.0 | plans/fnv.md | Faster environment HashMap lookups |
| smallvec | 1.15 | plans/smallvec.md | Stack-allocated small vectors for attrs/args |
| log | 0.4 | plans/log-simplelog.md | Interpreter diagnostic logging |
| env_logger | 0.11 | plans/log-simplelog.md | Logging backend (logging feature) |

### Have plans but not yet vendored

| Crate | Plan | R feature |
| ----- | ---- | --------- |
| polars | plans/polars-dataframe.md | Data frame backend |
| egui_table + eframe | plans/egui-table-view.md | View() spreadsheet display |
| chrono | plans/chrono.md | Alternative to jiff for date/time (vendored but superseded) |

## Infrastructure Crates (no R relevance)

These are build tools, proc-macro plumbing, platform bindings, and transitive deps. No plan needed.

### Build / compilation

autocfg, cc, find-msvc-tools, prettyplease, rustc_version, rustversion, semver, shlex, version_check

### Proc-macro plumbing

convert_case, document-features, heck, linkme-impl, litrs, proc-macro2, quote, strum, strum_macros, syn, derive_more-impl, thiserror-impl, serde_derive, serde_core, miette-derive, winnow

### Platform bindings

android_system_properties, core-foundation, core-foundation-sys, errno, libc, libm, linux-raw-sys, r-efi, redox_syscall, redox_users, rustix, winapi, winapi-i686-pc-windows-gnu, winapi-util, winapi-x86_64-pc-windows-gnu, windows-core, windows-implement, windows-interface, windows-link, windows-result, windows-strings, windows-sys, windows-sys-0.52.0, windows-sys-0.59.0, windows-sys-0.60.2, windows-targets, windows-targets-0.52.6, windows_aarch64_gnullvm, windows_aarch64_gnullvm-0.52.6, windows_aarch64_msvc, windows_aarch64_msvc-0.52.6, windows_i686_gnu, windows_i686_gnu-0.52.6, windows_i686_gnullvm, windows_i686_gnullvm-0.52.6, windows_i686_msvc, windows_i686_msvc-0.52.6, windows_x86_64_gnu, windows_x86_64_gnu-0.52.6, windows_x86_64_gnullvm, windows_x86_64_gnullvm-0.52.6, windows_x86_64_msvc, windows_x86_64_msvc-0.52.6, crossterm_winapi

### Compression (transitive deps of flate2)

adler2, crc32fast, miniz_oxide, simd-adler32

### Crypto internals (transitive deps of sha2, ring, rustls)

block-buffer, chacha20, cpufeatures, cpufeatures-0.2.17, crypto-common, digest, generic-array, ring, subtle, typenum, untrusted, zeroize

### Parser internals (transitive deps of pest, toml_edit)

pest_derive, pest_generator, pest_meta, ucd-trie, toml_datetime, toml_write

### Regex internals

regex-automata, regex-syntax

### Concurrency primitives (transitive deps of rayon, parking_lot)

crossbeam-deque, crossbeam-epoch, crossbeam-utils, lock_api, once_cell, once_cell_polyfill, parking_lot, parking_lot_core, portable-atomic, portable-atomic-util, scopeguard

### I/O and terminal

anstream, anstyle, anstyle-parse, anstyle-query, anstyle-wincon, colorchoice, crossterm, fd-lock, is_ci, is_terminal_polyfill, mio, signal-hook-0.3.18, signal-hook-mio, signal-hook-registry, strip-ansi-escapes, supports-color, supports-hyperlinks, supports-unicode, vte, utf8parse

### TLS (transitive deps of rustls)

openssl-probe, ring, rustls-native-certs, rustls-pki-types, rustls-webpki, schannel, security-framework, security-framework-sys, webpki-roots, webpki-roots-0.26.11

### Numeric (transitive deps of ndarray, rand)

getrandom, getrandom-0.2.17, matrixmultiply, num-integer, num-traits, rand_core, rawpointer

### String / text utilities

aho-corasick, bytecount, itoa, ryu, unicode-ident, unicode-linebreak, unicode-xid, zmij

### Data structures

bitflags, bumpalo, either, equivalent, foldhash, hashbrown, hashbrown-0.15.5, id-arena, smallvec

### Time (transitive deps of jiff, chrono)

chrono, iana-time-zone, iana-time-zone-haiku, jiff-static, jiff-tzdb, jiff-tzdb-platform

### Serialization

csv-core, serde, serde_core, serde_derive

### Display

nu-ansi-term, owo-colors, papergrid, testing_table, textwrap, terminal_size, unicode-width-0.1.14

### Logging

env_filter, env_logger, log

### Error handling

anyhow

### Directory utilities

dirs-sys, libredox, option-ext, same-file

### WASM (transitive deps, not relevant)

js-sys, leb128fmt, wasi, wasip2, wasip3, wasm-bindgen, wasm-bindgen-macro, wasm-bindgen-macro-support, wasm-bindgen-shared, wasm-encoder, wasm-metadata, wasmparser, wit-bindgen, wit-bindgen-core, wit-bindgen-rust, wit-bindgen-rust-macro, wit-component, wit-parser

### Misc

cfg-if, itertools-0.13.0, log, memchr
