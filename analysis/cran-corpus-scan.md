# CRAN Corpus Scan

Scan of the checked-in `cran/` corpus to identify the runtime components miniR needs in order to execute and incorporate real packages instead of only parsing them.

Last updated: 2026-03-15
Tree directories: 234
`DESCRIPTION`-bearing packages: 222

## Scope

- Parsed `DESCRIPTION` metadata for package dependencies and `LinkingTo`.
- Scanned `NAMESPACE` files for `S3method()` and `useDynLib()`.
- Counted package assets such as `src/`, `man/`, `inst/doc/`, and `vignettes/`.
- Counted native source file types, `inst/include` headers, and native routine registration patterns.
- Performed lightweight token scans over `R/`, `src/`, and `tests/` for runtime signals such as package loading, native calls, serialization, graphics, connections, and date/time helpers.

The dependency/runtime counts below use the 222 packages that ship `DESCRIPTION`. Asset counts sometimes use all 234 top-level package directories because the checked-in base/recommended packages in `cran/` do not all carry a CRAN-style `DESCRIPTION` file in this tree.

These counts are presence signals, not exact execution counts, but they are good enough to rank the major compatibility components.

## Headline Findings

- The local CRAN corpus is already runtime-heavy, not parser-heavy.
- "Package loading" is too narrow a label for the remaining work. The corpus needs a full package runtime story: namespaces, imports/exports, hooks, datasets, search path behavior, and base/recommended package environments.
- Native extension support is required for a large slice of the corpus, not just niche packages.
- Data/model fidelity still matters as much as builtin breadth: attributes, `data.frame`, factors, call semantics, and dispatch show up everywhere.

## Dependency Signals

### Most common imported packages

| Package | Import count |
| ------- | ------------ |
| utils | 76 |
| rlang | 61 |
| methods | 45 |
| cli | 45 |
| stats | 44 |
| lifecycle | 40 |
| graphics | 25 |
| glue | 25 |
| jsonlite | 23 |
| grDevices | 22 |
| magrittr | 21 |
| R6 | 21 |
| vctrs | 21 |
| tibble | 19 |
| withr | 19 |

### Base and recommended runtime packages referenced through imports/depends

| Package | Packages referencing it |
| ------- | ----------------------- |
| utils | 122 |
| stats | 92 |
| methods | 61 |
| graphics | 42 |
| grDevices | 39 |
| tools | 22 |
| grid | 15 |

## Package Asset Signals

| Asset | Count | Notes |
| ----- | ----- | ----- |
| top-level package directories | 234 | full checked-in `cran/` tree |
| `DESCRIPTION` files | 222 | contrib-style metadata-bearing packages |
| `NAMESPACE` files | 230 | package namespace metadata is nearly universal |
| `src/` directories | 128 | compiled code is common |
| `man/` directories | 234 | every checked-in package directory ships documentation |
| `man/*.Rd` files | 10,738 | help alias/indexing surface is large |
| `inst/doc/` directories | 125 | built documentation and vignettes are common |
| `vignettes/` directories | 126 | source vignettes are common |

## Native Source Mix

The native package surface is mostly conventional C/C++/Fortran shared-library code, with a long tail of vendored upstream build systems and platform-specific sources.

| Source type | Count | Notes |
| ----------- | ----- | ----- |
| `.c` | 3,382 | dominant native language |
| `.h` | 2,496 | native headers under `src/` |
| `.hpp` | 1,652 | C++ headers and vendored header-only deps |
| `.cpp` | 1,239 | major C++ surface |
| `.cc` | 164 | smaller C++ surface |
| `.f` | 128 | Fortran 77 still matters |
| `.f90` | 41 | modern Fortran also appears |

Long-tail native/build artifacts also appear in `src/`: `.m`, `.mm`, `.cu`, `.cuh`, `.y`, `.l`, `.rl`, `.asm`, `.s`, `Makevars`, `.def`, `.cmake`, and other vendored build files. miniR does not need first-class parsers for those file types, but package installation needs to preserve them and drive the platform toolchain correctly.

## Header and `inst/include` Signals

Package incorporation also needs to preserve compile-time headers for downstream packages.

| Signal | Count | Notes |
| ------ | ----- | ----- |
| packages with `inst/include` | 29 | compile-time headers are common enough to be a first-class concern |
| header-like files under `inst/` | 15,766 | dominated by header-only libraries such as Boost/Eigen |

Representative packages with `inst/include`:

- `BH`
- `Matrix`
- `Rcpp`
- `RcppEigen`
- `RcppParallel`
- `StanHeaders`
- `cpp11`
- `data.table`
- `sf`
- `systemfonts`
- `vctrs`
- `xml2`

Implication:

- package installation/loading must preserve all of `inst/`
- `inst/include` must be exposed to downstream package compilation for `LinkingTo:`
- the interpreter's package model must treat headers as installed package assets, not incidental files

## Runtime Signals

| Signal | Count | What it implies |
| ------ | ----- | --------------- |
| package loading / namespace usage | 197 | `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, `:::` |
| S3 registrations in `NAMESPACE` | 162 | working package namespaces plus stable S3 dispatch |
| namespace hooks | 139 | `.onLoad()`, `.onAttach()`, `.onUnload()` support |
| native source trees | 120 | compiled package support matters |
| `.Call()` usage | 120 | native routine lookup and invocation are necessary |
| `useDynLib()` directives | 116 | package DLL loading and registration |
| serialization calls | 129 | `readRDS()`, `saveRDS()`, `load()`, `save()`, lazy data |
| connection / I/O calls | 163 | `file()`, `url()`, `gzfile()`, `pipe()`, downloads, sockets |
| process / filesystem state | 135 | `tempdir()`, `tempfile()`, env vars, working directory, paths |
| attribute / object helpers | 207 | `attributes()`, `attr()`, `structure()`, `class()`, `dim()` fidelity |
| `data.frame`-style code | 164 | `data.frame`, `as.data.frame`, subsetting/coercion behavior |
| explicit S4 / methods code | 39 | `setClass()`, `setMethod()`, `setGeneric()`, `methods` package integration |
| graphics calls | 126 | plotting, devices, `grid`, `ggplot2`, graphics state |
| graphics device helpers | 59 | `pdf()`, `png()`, `svg()`, `dev.off()` |
| date/time helpers | 96 | `Date`, `POSIXct`, `POSIXlt`, parsing, formatting, time zones |
| R6 usage | 33 | reference-style OO support in some higher-level packages |

## Native Loading and ABI Signals

The corpus expects GNU R's native routine model, not just ad hoc `dlopen()` of arbitrary functions.

| Signal | Count | What it implies |
| ------ | ----- | --------------- |
| `useDynLib(...)` in `NAMESPACE` | 123 packages | package libraries are declared as part of namespace loading |
| R/tests files containing `.Call()` | 862 | this is the primary native call path |
| R/tests files containing `.C()` | 91 | legacy vector-copy ABI still appears |
| R/tests files containing `.External()` | 39 | varargs-style native entrypoints still appear |
| R/tests files containing `.Fortran()` | 36 | Fortran bridge support is still needed |
| source files containing `R_registerRoutines()` | 129 | registered routine tables are the norm |
| source files containing `R_useDynamicSymbols(..., FALSE)` | 120 | packages often require symbol lookup to go through registration |
| source files containing `R_forceSymbols(..., TRUE)` | 51 | symbol objects / stricter lookup matter for some packages |
| source files containing `R_RegisterCCallable()` | 28 | packages export C-callable APIs to other packages |
| source files containing `R_GetCCallable()` | 14 | cross-package C API lookup is in use |

## Required Components

### 1. Package and namespace runtime

This is the biggest missing compatibility layer. Making `library()` exist is not enough.

Necessary pieces:

- Parse `DESCRIPTION` and `NAMESPACE`.
- Build real namespace and package environments.
- Support `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, and `:::`.
- Run namespace hooks such as `.onLoad()` and `.onAttach()`.
- Model the search path and package attachment/detachment behavior.
- Load package datasets and lazy data metadata.

Why this is required:

- 197 packages reference package-loading or namespace operations.
- 162 packages register S3 methods in `NAMESPACE`.
- 139 packages define namespace hooks.

### 2. Native extension loading and FFI

The corpus includes many packages with `src/` trees and explicit native calls, so package incorporation cannot stop at sourcing `R/` files.

Necessary pieces:

- Compile conventional package `src/` trees with the platform C/C++/Fortran toolchain.
- Compile and stage package `src/` trees as part of package installation/loading.
- Preserve and expose `inst/include` for packages that are used via `LinkingTo:`.
- Load package shared libraries declared via `useDynLib()`.
- Create interpreter-local `DllInfo`-style records for loaded libraries and registered routines.
- Support registered native routines rather than relying on process-global symbol lookup.
- Implement `.Call()` first, then `.External()`, `.C()`, and `.Fortran()`.
- Support `R_registerRoutines()`, `R_useDynamicSymbols()`, and `R_forceSymbols()`.
- Support `R_RegisterCCallable()` / `R_GetCCallable()` for cross-package C APIs.
- Keep native-library handles and lookup state on the interpreter instance, not in process-global statics.

Why this is required:

- 128 top-level package directories contain `src/`.
- 120 `DESCRIPTION`-bearing packages contain native source.
- 29 packages ship `inst/include`.
- 123 packages declare `useDynLib()`.
- 862 R/tests files contain `.Call()`.
- 129 source files register routines explicitly.
- 36 packages use `LinkingTo`.

### 3. Base and recommended package namespaces beyond `base`

A large fraction of the corpus assumes `utils`, `stats`, `methods`, `graphics`, `grDevices`, `grid`, and `tools` are importable as packages, not just as loose builtins.

Necessary pieces:

- Loadable namespaces for core packages.
- Enough exported functions and data objects for imports to resolve.
- Correct package environment names such as `package:stats` and `package:graphics`.

Why this is required:

- 122 packages reference `utils`.
- 92 reference `stats`.
- 61 reference `methods`.
- 42 reference `graphics`.
- 39 reference `grDevices`.

### 4. Object semantics, dispatch, and modeling surface

The corpus still leans heavily on R's object model and call semantics.

Necessary pieces:

- Stable S3 dispatch with package-aware generics and methods.
- `methods` package integration and S4 basics (`setClass`, `setMethod`, `setGeneric`).
- Better argument matching, call frames, and metaprogramming fidelity.
- Formula/model helpers needed by modeling packages: `terms`, `model.frame`, `model.matrix`, and related helpers.

Why this is required:

- 162 packages register S3 methods.
- 39 packages contain explicit S4 or `methods` definitions.
- 176 packages use metaprogramming helpers such as `eval()`, `substitute()`, `do.call()`, or `match.call()`.

### 5. Data-model fidelity

Many packages will still fail even after loading correctly if vectors, attributes, and data frames are not close enough to R.

Necessary pieces:

- Preserve and propagate `names`, `dim`, `dimnames`, `class`, and other attributes correctly.
- Finish `data.frame` subsetting/coercion edge cases.
- Keep factor behavior and level handling stable through indexing and combination.

Why this is required:

- 207 packages use attribute/object helpers.
- 164 packages use `data.frame`-style operations.
- 98 packages use factor helpers.

### 6. Runtime I/O, process-local state, and serialization

The corpus expects package code to touch files, temporary paths, connections, and serialized state routinely.

Necessary pieces:

- File and URL connections: `file()`, `url()`, `gzfile()`, `pipe()`, and friends.
- Better `readRDS()`, `saveRDS()`, `load()`, and `save()` compatibility.
- Stable per-interpreter working directory, environment variables, temp paths, and options.
- Enough network and filesystem behavior for package tests and helper code.

Why this is required:

- 163 packages use connection or I/O helpers.
- 135 packages use filesystem or process-state helpers.
- 129 packages use serialization helpers.

### 7. Package assets, documentation, and help indexing

The checked-in corpus does not just contain executable code. It also ships documentation assets that any serious package-incorporation story should preserve and index.

Necessary pieces:

- Treat package loading/installation as asset staging, not just `source()` over `R/`.
- Preserve package assets such as `man/`, `inst/`, `data/`, and compiled libraries.
- Parse or index `man/*.Rd` so `help()`, `?topic`, package help pages, aliases, and example lookup have something real to resolve against.
- Start with a metadata-first Rd parser/indexer for `\name`, `\alias`, `\title`, `\description`, `\usage`, `\examples`, `\keyword`, and `\docType`.
- Treat a full Rd parser as a staged project: package help lookup first, richer rendering/macros later.
- Keep package metadata wired up so documentation can refer back to package namespaces and installed assets.

Why this is required:

- All 234 checked-in package directories contain `man/`.
- The tree contains 10,738 `.Rd` files.
- 125 packages contain `inst/doc/`.
- 126 packages contain `vignettes/`.

This is not the first blocker for executing package code, but it is necessary if miniR is going to incorporate packages as packages rather than as loose script bundles.

On parser choice:

- A Pest-based Rd parser is a reasonable direction because miniR already uses Pest and already lowers `?topic` syntax into `help("topic")`.
- It should not start as a full GNU-R-compatible renderer. Rd is a macro language with conditionals and `\\Sexpr`, so the first useful deliverable is a stable help index and partial parse tree for lookup/rendering.

### 8. Graphics stack

Graphics are not optional for this corpus; they just do not outrank the package runtime and data model.

Necessary pieces:

- Basic `graphics` and `grDevices` device model.
- Enough `grid` support for packages built on top of it.
- Plot-state helpers such as `par()` and device lifecycle functions.

Why this is required:

- 126 packages reference graphics helpers.
- 59 packages reference device helpers such as `pdf()` or `dev.off()`.
- 42 packages reference `graphics`, 39 reference `grDevices`, and 15 reference `grid`.

### 9. Date/time and time zones

Date/time support is common enough that it should be treated as core runtime infrastructure.

Necessary pieces:

- `Date`, `POSIXct`, and `POSIXlt`.
- Parsing and formatting (`strptime()`, `strftime()`).
- Time-zone-aware conversions.

Why this is required:

- 96 packages reference date/time helpers.
- The corpus includes `lubridate`, `timeDate`, `timechange`, and `tzdb`.

### 10. Package build/install pipeline

The corpus shape implies that package support eventually needs a real staging pipeline rather than a one-off loader.

Necessary pieces:

- Build package directories into an installed layout that preserves `R/`, compiled libraries, `data/`, `man/`, `inst/`, and metadata.
- Feed package compilation the right header search paths, especially `inst/include` from `LinkingTo:` dependencies.
- Maintain an index for namespace metadata, help aliases, and installed package assets.
- Keep the install/load model compatible with the reentrant interpreter design by storing package state on the interpreter instance.

Why this is required:

- The package tree contains executable R code, compiled source, documentation, and installed docs/vignettes in the same package directories.
- A loader that ignores `src/` or `man/` will not be able to represent these packages faithfully.

## Prioritization Implications

- Parser completeness is no longer the main blocker for `cran/`.
- Adding more isolated builtins will not unlock the corpus unless the package runtime, native loading, package asset staging, and data semantics land with them.
- Full package installation UX can trail runtime loading for already-present source trees; the corpus first needs to load packages and run their R/native code correctly.
