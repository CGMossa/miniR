# Rd Parser And Package Runtime Crate Survey

Survey of Rust crates that might help with miniR's package runtime, native loading, and Rd parsing work.

Last updated: 2026-03-15

## Goal

Record the crate landscape around three problems:

- runtime loading of package shared libraries
- package build/install orchestration
- parsing `man/*.Rd`

This note is intentionally broader than a shortlist. It includes crates that are good fits, partial fits, and likely bad fits so we do not keep rediscovering the same options.

## Main Conclusion

No single crate solves miniR's package runtime problem.

What crates can help with:

- loading shared libraries
- invoking toolchains and locating external tools
- parsing/tokenizing documentation inputs
- emitting parser diagnostics

What remains miniR-specific:

- `DESCRIPTION` / `NAMESPACE`
- package install layout and asset staging
- `LinkingTo:` / `inst/include`
- `DllInfo`-style state
- routine registration and symbol policy
- `.Call()` / `.External()` / `.C()` / `.Fortran()`
- C-callable package APIs
- Rd semantics, indexing, and rendering decisions

## Category 1: Dynamic Loading

These crates are relevant for loading package shared libraries at runtime.

### `libloading`

Most likely default choice.

- cross-platform shared library loading
- symbol lookup
- mainstream ecosystem choice
- good fit for opening package DSOs and finding `R_init_pkg`

Use for:

- runtime package DLL/so/dylib loading
- raw symbol resolution before miniR wraps results in interpreter-local state

### `dlopen2`

Typed wrapper over dynamic loading primitives.

- more ergonomic than raw symbol lookup
- useful if we want a more structured wrapper around loaded library entry points

Use for:

- alternative to `libloading` if typed APIs feel cleaner

### `dlopen-rs` / `dlopen`

Lower-level dynamic loading interfaces.

- closer to raw `dlopen`/`LoadLibrary` style
- less attractive than `libloading` for general use, but still relevant

Use for:

- fallback if we want lower-level control

### `dlib`

Older small helper around dynamic loading.

- simpler / older option
- less compelling than `libloading`

### `libloader`, `libloading-mini`, similar wrappers

Minor ecosystem variants.

- probably not the default choice
- worth knowing they exist

## Category 2: Build And Process Orchestration

These crates may help the package installer invoke compilers, linkers, or external helpers.

### `cc`

Useful, but not sufficient by itself.

- build-time helper for invoking C/C++ compilation
- designed around Cargo build scripts more than runtime package installation

Good at:

- small controlled native compilation tasks

Not enough for:

- full CRAN-style package installation
- `Makevars` handling
- runtime compilation flow
- Fortran-heavy package installs as a complete solution

### `cmake`

Relevant only for packages that vendor CMake-based code.

- not the default CRAN compilation path
- useful for vendored upstream projects in some packages

### `subprocess`

Straightforward process management crate.

- useful for invoking compilers or shell tools
- relevant for installer orchestration

### `xshell`

Ergonomic command execution and shell-like scripting from Rust.

- attractive for package installer code if we want simple external-tool orchestration

### `duct`

Composable subprocess pipelines.

- useful if the installer ends up needing shell-like command composition

### `command-group`

Process-group control.

- relevant if package builds need cancellation or cleanup across child process trees

### `which`

Executable discovery.

- useful for finding `cc`, `c++`, `gfortran`, `make`, `pkg-config`, and similar tools

### `shell-words` / `shellwords`

Shell-style tokenization and quoting helpers.

- useful if we need to interpret or emit shell-ish command fragments

### `bindgen`

Generates Rust FFI bindings from C headers.

- useful only if miniR wants Rust bindings to native helper libraries
- not required for normal package loading

### `pkg-config`

Native dependency discovery.

- useful if some packages rely on system libraries exposed through `pkg-config`

### `vcpkg`

Windows native dependency integration.

- relevant mostly for Windows package-install story

## Category 3: Parser / Lexer Candidates For Rd

No off-the-shelf crate appears to implement GNU Rd. So the real choice is parser architecture, not "which Rd crate".

### Lexer-first approach

These are best considered as building blocks beneath a miniR-specific Rd parser.

### `logos`

Very good lexer generator.

- strong fit for tokenizing Rd commands, escapes, braces, and text
- useful if we want a fast dedicated lexer with explicit token kinds

Best use:

- front-end lexer for any custom Rd parser

### Hand-written lexer

Not a crate, but likely the strongest option.

- gives full control over lexer modes
- avoids bending a general parser framework around Rd's macro syntax

Best use:

- most likely long-term best fit for Rd

### `pest`

Already in the repo and already familiar.

- acceptable for a narrow metadata parser
- not obviously the best fit for the full Rd language

Pros:

- existing team familiarity
- easy integration for small grammars

Cons:

- less natural for mode-heavy macro parsing
- can become awkward for partial recovery and rich lexer state

### `peg`

Alternative PEG parser.

- similar tradeoffs to Pest
- worth noting, not clearly better here

### `chumsky`

Rust-native parser combinators with good ergonomics.

- attractive if we want expressive parser code and richer errors
- still probably wants a custom lexer beneath it

### `winnow`

Modern parser combinator library.

- good if we want explicit parsing over token streams
- stronger fit than PEG for controlled incremental parsing, but still not an obvious silver bullet

### `nom`

Classic Rust parser combinator library.

- powerful and widely used
- often lower-level than we need for a doc language

### `combine`

Older combinator library.

- still a plausible option
- less compelling than newer choices unless a specific API style appeals

### `pom`

Smaller parser combinator library.

- possible, but not an obvious advantage for Rd

### `LALRPOP`

Strong parser-generator option.

- good if we want a more formal grammar layer above a custom lexer
- plausible choice for a fuller Rd parser after indexing lands

### `lrlex` + `lrpar`

Lexer/parser generator stack close to classic lex/yacc style.

- especially relevant because GNU R's own Rd implementation is parser-generator based
- strong candidate if we want a generated parser rather than hand-written recursive descent

### `parol`

Another parser-generator option.

- worth considering if generated parsers become attractive

### `rustemo`

Grammar-driven parser framework.

- relevant as part of the wider landscape
- not obviously better than `LALRPOP` or `lrlex`/`lrpar`

### `tree-sitter`

Incremental syntax tree engine.

- probably not the right default for package-help ingestion
- more attractive for editor tooling than runtime help parsing

## Category 4: Diagnostics

These are useful once miniR has an Rd parser and wants good error messages.

### `ariadne`

High-quality annotated source diagnostics.

- strong fit for parser errors

### `annotate-snippets`

Lightweight annotated errors.

### `codespan-reporting`

Stable compiler-style reporting library.

### `miette`

High-level diagnostics framework.

- already has a plan in the repo
- useful if Rd parsing errors should share the same diagnostic style as other parser errors

## Best Fits By Subproblem

### Shared-library loading

Best fit:

- `libloading`

Acceptable alternatives:

- `dlopen2`
- `dlopen-rs`

### Package build/install orchestration

Best fit:

- mostly custom miniR logic
- plus process/tool discovery helpers such as `xshell`, `subprocess`, and `which`

Useful supporting crates:

- `cc` for narrow compile invocations
- `cmake` for vendored CMake projects
- `pkg-config` / `vcpkg` for native dependency discovery where needed

### Rd parsing

Best fit for phase 1:

- hand-written lexer + hand-written metadata parser

Best fit for a fuller parser:

- hand-written lexer + `lrlex` / `lrpar`
- hand-written lexer + `LALRPOP`
- hand-written lexer + recursive descent

Acceptable but not preferred:

- `pest`
- `peg`
- `chumsky`
- `winnow`

## Recommendation

### For dynamic loading

Use `libloading` unless a concrete requirement appears that points elsewhere.

### For package installation

Do not model the problem as "find one crate that installs CRAN packages".

Instead:

- keep the installer/package model custom
- use helper crates only for subprocess management, path/tool discovery, and optional native build integration

### For Rd parsing

Do not default to Pest just because miniR already uses Pest for R syntax.

Recommended order:

1. install-layout support for `man/`
2. alias/help index
3. hand-written or lexer-first metadata parser
4. basic `help()` integration
5. fuller parser later if rendering depth becomes worthwhile

### Why not start with a full GNU-R-compatible parser

- the early value is in lookup and indexing, not perfect rendering
- Rd has macro and stage semantics that make "full compatibility first" expensive
- package execution should not be blocked on the documentation renderer

## Relation To Existing Plans

- `plans/rd-parser.md` should drive implementation order for package help
- `analysis/cran-corpus-scan.md` describes why `man/`, `inst/`, `src/`, and `inst/include` all matter together
- `plans/implementation-plan.md` and `plans/interpreter-roadmap.md` capture the package-runtime priorities
