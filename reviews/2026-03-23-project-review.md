# 2026-03-23 Project Review

Scope: static review of the current runtime/package-loading implementation and adjacent embedding behavior. I did not run `cargo test`/`clippy` in this pass.

## Findings

### 1. Package-loading builtins do not implement R's calling convention, so common `library()` / `require()` forms will fail

Severity: high

- `library`, `require`, `loadNamespace`, and `requireNamespace` are registered as `#[interpreter_builtin]`, so their arguments are evaluated before the handler sees them: [src/interpreter/call_eval.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/call_eval.rs#L100), [src/interpreter/builtins.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins.rs#L3449), [src/interpreter/builtins.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins.rs#L3475), [src/interpreter/builtins.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins.rs#L3544), [src/interpreter/builtins.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins.rs#L3567).
- `extract_package_name()` then accepts only an already-evaluated character scalar and ignores the rest of the R signature: [src/interpreter/builtins.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins.rs#L3506).
- That breaks standard R call shapes such as `library(stats)` and `require("Matrix", .Library)`, both of which already appear in the checked-in corpus: [tests/reg-tests-1e.R](/Users/elea/Documents/GitHub/newr/tests/reg-tests-1e.R#L636), [tests/eval-etc-2.R](/Users/elea/Documents/GitHub/newr/tests/eval-etc-2.R#L12).

Why this matters: package-loading syntax is deliberately non-standard in R. Evaluating the package argument first changes semantics, and silently ignoring `lib.loc` removes a major compatibility escape hatch for installed packages.

### 2. The runtime's package lookup path disagrees with `.libPaths()`, so packages installed in the advertised default location are invisible to `library()`

Severity: high

- `Interpreter::get_lib_paths()` only includes `R_LIBS` and `R_LIBS_USER`: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L66).
- The public `.libPaths()` builtin documents and returns an additional default path, `<data_dir>/miniR/library`: [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1137), [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1187).
- Package discovery helpers all rely on `get_lib_paths()`: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L54), [src/interpreter/builtins/system.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/system.rs#L1935), [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L5496).

Why this matters: the interpreter can tell the user a package is on `.libPaths()` and then fail to load, inspect, or locate files from that same package.

### 3. Attached-package bookkeeping is out of sync with the actual environment chain, so `search()` / `utils::find()` can report the wrong lookup order

Severity: medium

- `attach_package()` inserts each new package directly between `.GlobalEnv` and the current parent, making the newest package the first one searched: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L493).
- But the side `search_path` list is updated with `push()`, which preserves attach order instead of effective lookup order: [src/interpreter/packages/loader.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/packages/loader.rs#L499).
- `search()` and `find()` read that side list rather than walking the real parent chain: [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L5244), [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L5337).

Why this matters: after attaching `pkg1` and then `pkg2`, name resolution goes through `pkg2` first, but `search()`/`find()` will still claim `pkg1` is ahead of it. That makes debugging package conflicts much harder.

### 4. Namespace helper builtins are still stubbed or semantically wrong even though a real namespace loader now exists

Severity: medium

- `asNamespace()` always returns `NULL`: [src/interpreter/builtins/stubs.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/stubs.rs#L263).
- `isNamespace()` returns `TRUE` for any environment, not just namespace environments: [src/interpreter/builtins/stubs.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/stubs.rs#L293).
- At the same time there is already a real `getNamespace()` implementation backed by `loaded_namespaces` and on-demand loading: [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L5378).

Why this matters: package code and tests already use `asNamespace(...)`, so this is not a theoretical gap: [tests/reg-translation.R](/Users/elea/Documents/GitHub/newr/tests/reg-translation.R#L44).

### 5. Stub warnings bypass session-scoped stderr and leak to the process-global terminal

Severity: medium

- Stub warnings use `eprintln!` directly instead of the interpreter/session writer: [src/interpreter/builtins/stubs.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/stubs.rs#L8).
- Captured-output sessions explicitly route interpreter stderr into an in-memory buffer: [src/session.rs](/Users/elea/Documents/GitHub/newr/src/session.rs#L102).

Why this matters: embedded users and tests cannot reliably capture or suppress these messages, which works against the reentrant/session-local design the project is otherwise following.

## Assumptions

- This was a read-only/static review pass.
- I did not re-run the Rust test suite or lint suite from this environment.
