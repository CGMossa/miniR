+++
title = "Package Runtime"
weight = 5
description = "How miniR finds packages, loads namespaces, sources R files, and attaches exports"
+++

miniR treats package loading as a runtime subsystem, not as a thin wrapper around `source()`. The package loader is responsible for finding packages, building namespace environments, importing symbols, loading native code, sourcing `R/` files, indexing help, and attaching exports onto the search path.

## The Loading Sequence

The core loader lives in `src/interpreter/packages/loader.rs`. The rough order is:

1. Find the package directory on `.libPaths()`
2. Parse `DESCRIPTION`
3. Parse `NAMESPACE`
4. Create a namespace environment with the base environment as parent
5. Populate imports from already-loaded packages
6. Load native code declared by `useDynLib()` before sourcing R code
7. Load `R/sysdata.rda` if present
8. Source `R/*.R` files, respecting `Collate`
9. Build a filtered exports environment
10. Register `S3method()` entries
11. Run `.onLoad()`
12. If the caller used `library()` or `require()`, attach the exports environment and then run `.onAttach()`

That is why package compatibility work in miniR is usually about runtime semantics, not only parser coverage.

## Where Packages Are Found

Library paths are built from interpreter-local environment state:

- `R_LIBS`
- `R_LIBS_USER`
- miniR's default library directory under the user data dir

This logic is in `Interpreter::get_lib_paths()`. The paths are resolved against the interpreter's working directory and environment variables, both of which are also stored on the interpreter instance.

## Namespace Versus Search Path

miniR keeps these distinct:

| Concept | What it is |
|------|-------------|
| Namespace environment | All package code and internal objects |
| Exports environment | The user-visible subset of the namespace |
| Search path entry | An attached exports environment inserted between `.GlobalEnv` and `base` |

`load_namespace()` builds the namespace. `attach_package()` inserts the exports environment into the parent chain and records it in the interpreter's search-path list.

## Built-In Base Packages

Packages such as `base`, `stats`, `utils`, `methods`, `graphics`, `grDevices`, and `grid` are treated specially. miniR registers synthetic namespaces for them instead of expecting installable package directories.

That means calls like `library(base)` and `loadNamespace("stats")` can still work even though those packages are backed by builtin code and interpreter state.

## DESCRIPTION And NAMESPACE Matter

The package runtime does real metadata work:

- `DESCRIPTION` drives package identity, dependencies, imports, and collate order.
- `NAMESPACE` drives exports, imports, `importFrom()`, `S3method()`, and `useDynLib()`.
- `man/` directories are indexed into the Rd help store.

If a package fails because a symbol is missing or a method is not visible, the problem is often in this metadata path rather than in the parser.

## R File Sourcing

miniR sources package `R/` files in collate order when `DESCRIPTION` provides one. Files not listed in `Collate` are sourced afterwards in alphabetical order.

There is one pragmatic divergence here: top-level expression failures while sourcing a file are logged and the loader continues with later expressions. That keeps packages usable when one top-level helper fails but later definitions, hooks, or methods still need to exist.

## Imports, Exports, And Hooks

The package loader:

- copies imported symbols into the namespace environment
- builds a filtered exports environment from `export()` and `exportPattern()`
- registers S3 methods declared in `NAMESPACE`
- calls `.onLoad()` after the namespace is ready
- calls `.onAttach()` only when the package is attached to the search path

This split matches how packages expect to behave in real R sessions.

## Native Code Is Part Of Package Loading

When `NAMESPACE` includes `useDynLib()`, miniR loads package native code before sourcing `R/` files. That allows package R code to refer to `.Call` targets during load.

This is one reason package compatibility and native-runtime work are tightly coupled in miniR.

## Where To Work When Package Loading Fails

| Symptom | Start here |
|------|-------------|
| Package not found | `packages/loader.rs`, `.libPaths()` logic |
| Wrong exports/imports | `packages/namespace.rs`, `build_exports()`, `populate_imports()` |
| Hook or load-order issue | `packages/loader.rs`, `Collate`, `.onLoad()`, `.onAttach()` |
| Missing S3 method | `packages/namespace.rs`, `register_s3_methods()`, `s3.rs` |
| Native symbol not available during load | `packages/loader.rs`, `native/dll.rs`, `native/runtime.rs` |

In practice, package runtime bugs are often environment-chain or metadata bugs. Treat them that way and the fixes stay coherent.
