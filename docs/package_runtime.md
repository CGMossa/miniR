# Package Runtime

miniR treats package loading as a runtime subsystem with its own environment model, metadata parsing, native loading path, and help indexing. This page is the reference view of that pipeline.

## Why The Package Loader Is A Core Runtime Layer

The package system is not a thin wrapper around `source()`. Real packages expect all of these to work together:

- `DESCRIPTION` and `NAMESPACE`
- imports and exports
- namespace environments versus attached search-path entries
- package hooks such as `.onLoad()` and `.onAttach()`
- `man/` indexes and help lookup
- native code declared through `useDynLib()`

If those pieces are even slightly wrong, package code usually fails far away from the real cause.

## Loading Sequence

The main loading path lives in `src/interpreter/packages/loader.rs`. The rough order is:

1. Find the package directory on `.libPaths()`.
2. Parse `DESCRIPTION`.
3. Parse `NAMESPACE`.
4. Create a namespace environment with the base environment as parent.
5. Resolve imports from already-loaded packages.
6. Load native code declared by `useDynLib()` before sourcing R files.
7. Load `R/sysdata.rda` if present.
8. Source package `R/` files in collate order.
9. Build an exports environment.
10. Register `S3method()` entries.
11. Run `.onLoad()`.
12. If the caller used `library()` or `require()`, attach the exports environment and run `.onAttach()`.

That order matters. Packages often assume imported symbols, registered methods, and native routines already exist by the time package code starts running.

## Library Paths And Interpreter State

Library resolution is interpreter-local. miniR builds package search paths from interpreter-owned environment state such as:

- `R_LIBS`
- `R_LIBS_USER`
- miniR's default user-library directory

This is important for reentrancy. Two interpreters in the same process can point at different library trees without racing on a process-global setting.

## Namespace Versus Search Path

miniR keeps namespace state separate from the attached user-facing environment:

| Concept | Meaning |
| ------- | ------- |
| Namespace environment | All package code and internal objects |
| Exports environment | The subset exposed to users and importing packages |
| Search path entry | An attached exports environment inserted into the environment chain |

`loadNamespace()` should build the namespace even if the package is never attached. `library()` and `require()` add the search-path effect on top of that.

## DESCRIPTION And NAMESPACE Work

The package runtime does real metadata work rather than treating package files as a flat script directory:

- `DESCRIPTION` drives package identity, dependency declarations, and collate order.
- `NAMESPACE` drives exports, imports, `importFrom()`, `S3method()`, and `useDynLib()`.
- `man/` directories are indexed into the interpreter's Rd help store.

When a package appears to be "missing a function", the bug is often in metadata handling rather than in the evaluator.

## Built-In Base Packages

Packages such as `base`, `stats`, `utils`, `methods`, `graphics`, `grDevices`, and `grid` are special. miniR does not expect installed package directories for them; it registers synthetic namespaces backed by builtin code and interpreter state.

That keeps calls like `library(base)` and `loadNamespace("stats")` meaningful even when those packages are not ordinary directories on disk.

## Collate, Sourcing, And Failure Model

miniR sources package `R/` files in `Collate` order when the package declares one. Files not listed there are loaded afterwards in alphabetical order.

There is one pragmatic divergence in this area: top-level expression failures while sourcing a package file are logged and later top-level expressions still run. The reason is practical package survivability. A single top-level failure should not automatically prevent later helper definitions, hooks, or method registrations from existing.

That policy should stay visible because it changes how partial package loads behave.

## S3 Registration And Hooks

The loader is also responsible for runtime bookkeeping that is easy to miss:

- importing symbols into the namespace
- building the filtered exports environment
- registering `S3method()` declarations
- calling `.onLoad()` after the namespace is ready
- calling `.onAttach()` only after attachment to the search path

Those are not independent extras. They are part of what makes package code behave like package code.

## Where To Debug Package Failures

Start in the package runtime when the symptom looks like:

- `library()` or `loadNamespace()` cannot find a package
- imported symbols are missing
- `S3method()` registrations do not stick
- `.onLoad()` or `.onAttach()` behaves at the wrong time
- help topics from `man/` do not show up
- package code works when sourced manually but not when loaded as a package

Most of those bugs are loader or namespace issues, not parser bugs.
