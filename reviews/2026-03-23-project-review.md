# 2026-03-23 Project Review

Scope: static review of the current runtime/package-loading implementation and adjacent embedding behavior. I did not run `cargo test`/`clippy` in this pass.

## Findings

### 1. Package-loading builtins do not implement R's calling convention ~~so common `library()` / `require()` forms will fail~~

Severity: ~~high~~ **RESOLVED** (2026-04-01)

`library`/`require` are now `#[pre_eval_builtin]` with NSE via `extract_package_name_nse()`:
- Bare symbols: `library(dplyr)` → uses symbol name
- String literals: `library("dplyr")` → uses string value
- `character.only=TRUE`: evaluates the argument (including variables)
- Base packages (base, stats, utils, methods, etc.) silently succeed as no-ops
- Base packages register synthetic `LoadedNamespace` entries for `getNamespaceExports()`/`asNamespace()` compatibility

### 2. The runtime's package lookup path disagrees with `.libPaths()`, so packages installed in the advertised default location are invisible to `library()`

Severity: high

- `Interpreter::get_lib_paths()` only includes `R_LIBS` and `R_LIBS_USER`: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L66).
- The public `.libPaths()` builtin documents and returns an additional default path, `<data_dir>/miniR/library`: [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1137), [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1187).
- Package discovery helpers all rely on `get_lib_paths()`: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L54), [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1935), [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L5496).

Why this matters: the interpreter can tell the user a package is on `.libPaths()` and then fail to load, inspect, or locate files from that same package.

### 3. ~~Attached-package bookkeeping is out of sync with the actual environment chain~~

Severity: ~~medium~~ **RESOLVED**

`attach_package()` now uses `insert(0, ...)` instead of `push()` for the search_path
list, keeping it in sync with the environment parent chain. Verified: loading cachem
then memoise produces correct search order `[.GlobalEnv, package:memoise, package:cachem, package:base]`.

### 4. ~~Namespace helper builtins are still stubbed or semantically wrong~~

Severity: ~~medium~~ **RESOLVED**

`asNamespace()` and `isNamespace()` now work correctly against the `loaded_namespaces`
registry. Base packages (stats, utils, etc.) register synthetic `LoadedNamespace` entries
with `export_all()` patterns, so `isNamespace(asNamespace("stats"))` returns TRUE.

### 5. Stub warnings bypass session-scoped stderr and leak to the process-global terminal

Severity: medium

- Stub warnings use `eprintln!` directly instead of the interpreter/session writer: [src/interpreter/builtins/stubs.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/stubs.rs#L8).
- Captured-output sessions explicitly route interpreter stderr into an in-memory buffer: [src/session.rs](/Users/elea/Documents/GitHub/newr/src/session.rs#L102).

Why this matters: embedded users and tests cannot reliably capture or suppress these messages, which works against the reentrant/session-local design the project is otherwise following.

## Assumptions

- This was a read-only/static review pass.
- I did not re-run the Rust test suite or lint suite from this environment.
