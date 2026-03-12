# Interpreter Roadmap

High-level roadmap for bringing miniR from “parses R” to “runs more real R code.”

## Current State (2026-03-12)

- Parsing is no longer the primary blocker: the grammar, AST, and custom parse errors are already in place.
- The runtime already has attributes, matrices/arrays, factors, regex helpers, language objects, a basic `data.frame()`, and substantial metaprogramming support.
- The remaining bottlenecks are now semantic and runtime-heavy: call-stack behavior, generic dispatch consistency, package/runtime surface area, and the embedding model.

## Next Priorities

1. Call-stack semantics
   - Real call frames for closures
   - `sys.*`, `parent.frame()`, `nargs()`, `missing()`, and `on.exit()` alignment

2. S3 dispatch consistency
   - Replace the direct `UseMethod()` noop path
   - Keep `NextMethod()` and call-stack introspection coherent inside dispatched methods

3. Complete the simplified object/data model
   - Finish `data.frame()` semantics
   - Keep attributes, names, and classes flowing correctly through more operations

4. Expand the runtime surface
   - Serialization, connections, package loading, and date/time support

5. Reentrant embedding cleanup
   - Move from a TLS-centric singleton usage pattern toward a true multi-instance public API

6. Long-tail compatibility work
   - S4, graphics, linear algebra decompositions, package installation, and lower-frequency builtins

## Working Rules

- Prefer flat priority order over phase plans.
- Use `TODO.md` for concrete remaining stubs and `DONE.md` for landed behavior.
- Refresh this roadmap whenever a major runtime foundation lands; do not let it describe already-shipped work as future work.
