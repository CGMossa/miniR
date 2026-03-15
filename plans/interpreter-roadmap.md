# Interpreter Roadmap

High-level roadmap for bringing miniR from “parses R” to “runs more real R code.”

## Current State (2026-03-15)

- Parsing is no longer the primary blocker: the grammar, AST, and custom parse errors are already in place.
- The runtime already has attributes, matrices/arrays, factors, regex helpers, language objects, a basic `data.frame()`, and substantial metaprogramming support.
- A scan of the checked-in `cran/` corpus (`analysis/cran-corpus-scan.md`) confirms that the remaining bottlenecks are semantic and runtime-heavy: package/namespace loading, native routine support, base package namespaces, object/data-model fidelity, graphics/I/O/date-time support, and the embedding model.

## Next Priorities

1. Call-stack semantics
   - Real call frames for closures
   - `sys.*`, `parent.frame()`, `nargs()`, `missing()`, and `on.exit()` alignment

2. Package and namespace runtime
   - `library()`, `require()`, `requireNamespace()`, `loadNamespace()`, `::`, `:::`
   - `DESCRIPTION` / `NAMESPACE` handling, search path behavior, package hooks, datasets, and installed package assets (`data/`, `inst/`, `man/`, `inst/include`)

3. Native extension loading
   - compile `src/` trees, honor `LinkingTo:` / `inst/include`, and support `useDynLib()`
   - registered routines, `DllInfo`-style library state, `.Call()` / `.External()` / `.C()` / `.Fortran()`, and C-callable registration
   - Interpreter-local native-library state for reentrant embedding

4. Dispatch and object/data-model fidelity
   - Keep S3 dispatch coherent inside package namespaces
   - Add `methods` / S4 basics and finish `data.frame` / attribute semantics

5. Expand the runtime surface
   - Connections, serialization, filesystem/temp/env behavior, graphics, date/time support, and Rd/help indexing
   - staged Rd parser/indexer for `help()` and `?topic`, not just a string lookup table
   - Core package namespaces beyond `base`: `utils`, `stats`, `methods`, `graphics`, `grDevices`, `grid`, `tools`

6. Reentrant embedding cleanup
   - Move from a TLS-centric singleton usage pattern toward a true multi-instance public API

7. Long-tail compatibility work
   - Graphics depth, help/doc UX polish, linear algebra decompositions, package installation UX, and lower-frequency builtins

## Working Rules

- Prefer flat priority order over phase plans.
- Use `TODO.md` for concrete remaining stubs and `DONE.md` for landed behavior.
- Refresh this roadmap whenever a major runtime foundation lands; do not let it describe already-shipped work as future work.
