# Interpreter Roadmap

High-level roadmap for bringing miniR from "parses R" to "runs more real R code."

## Current State (2026-03-15)

- Parsing is no longer the primary blocker: the grammar, AST, and custom parse errors are already in place.
- The runtime already has attributes, matrices/arrays, factors, regex helpers, language objects, a basic `data.frame()`, and substantial metaprogramming support.
- Call stack semantics, S3 dispatch, date/time, reentrancy, type stability, and argument matching are all shipped.
- All builtins use explicit `BuiltinContext` — zero TLS calls remain in the builtin layer.
- A scan of the checked-in `cran/` corpus (`analysis/cran-corpus-scan.md`) confirms that the remaining bottlenecks are semantic and runtime-heavy: package/namespace loading, native routine support, base package namespaces, object/data-model fidelity, graphics/I/O support, and help/documentation indexing.

## Shipped

- Call-stack semantics (sys.*, parent.frame, nargs, missing, on.exit)
- S3 dispatch (UseMethod, NextMethod, print/format as generics)
- Date/time (jiff-based: Date, POSIXct, POSIXlt, strptime, difftime)
- Reentrancy (BuiltinContext, per-interpreter env vars/cwd, thread isolation)
- Type stability (type-preserving indexing, assignment, arithmetic, attribute propagation)
- Three-pass argument matching (exact, partial, positional, unused-arg errors)
- Help system (?name from rustdoc comments, Builtin trait with FromArgs derive)
- 370+ documented builtins

## Next Priorities

1. Package and namespace runtime
   - `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, `:::`
   - `DESCRIPTION` / `NAMESPACE` handling, search path behavior, package hooks, datasets, and installed package assets (`data/`, `inst/`, `man/`, `inst/include`)

2. Data-model fidelity
   - Finish `data.frame` subsetting/coercion edge cases
   - `as.vector()` and attribute propagation through combination operations

3. Native extension loading
   - Compile `src/` trees, honor `LinkingTo:` / `inst/include`, and support `useDynLib()`
   - Registered routines, `DllInfo`-style library state, `.Call()` / `.External()` / `.C()` / `.Fortran()`, and C-callable registration
   - Interpreter-local native-library state for reentrant embedding

4. Base package namespaces beyond `base`
   - Core package namespaces: `utils`, `stats`, `methods`, `graphics`, `grDevices`, `grid`, `tools`

5. Expand the runtime surface
   - Connections, serialization, graphics, and Rd/help indexing
   - Staged Rd parser/indexer for `help()` and `?topic`

6. Long-tail compatibility work
   - S4 depth, graphics devices, linear algebra decompositions, package installation UX, and lower-frequency builtins

## Working Rules

- Prefer flat priority order over phase plans.
- Use `TODO.md` for concrete remaining stubs and `DONE.md` for landed behavior.
- Refresh this roadmap whenever a major runtime foundation lands; do not let it describe already-shipped work as future work.
