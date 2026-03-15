# Package and Namespace Runtime

The biggest remaining compatibility blocker. 197 of 222 CRAN packages in the
corpus reference package loading or namespace operations.

## What R's package system does

When `library(dplyr)` runs, R:

1. Finds the installed package directory (e.g. `lib/dplyr/`)
2. Reads `DESCRIPTION` for dependencies
3. Loads dependencies recursively
4. Creates a **namespace environment** for the package
5. Sources all `R/*.R` files into the namespace environment
6. Reads `NAMESPACE` to determine:
   - What to export (make visible to users)
   - What to import from other packages
   - S3 method registrations
   - `useDynLib` directives
7. Runs `.onLoad()` hook in the namespace
8. Creates a **package environment** (the user-visible one) containing only exports
9. Attaches the package environment to the **search path**
10. Runs `.onAttach()` hook

## Minimum viable implementation

### Phase A: DESCRIPTION + NAMESPACE parsing

- Parse `DESCRIPTION`: `Package`, `Depends`, `Imports`, `Suggests`, `LinkingTo`
- Parse `NAMESPACE`: `export()`, `exportPattern()`, `import()`, `importFrom()`,
  `S3method()`, `useDynLib()`
- These are simple DSLs, not R code — hand-written parsers are fine

### Phase B: Namespace environments

- Each package gets a namespace env with parent = base env
- `R/*.R` files are sourced into the namespace env
- Imports are resolved: `importFrom(rlang, sym)` binds `sym` in the namespace
- Exports create a package env that's a filtered view of the namespace

### Phase C: Search path and library()

- `library(pkg)` triggers the loading sequence
- Package env is attached to the search path (between global and base)
- `::` and `:::` resolve against package/namespace envs
- `require()` returns FALSE instead of erroring if package not found

### Phase D: Hooks and S3 registration

- Call `.onLoad(libname, pkgname)` after sourcing R/
- Call `.onAttach(libname, pkgname)` after attaching
- Register S3 methods declared in NAMESPACE

### Phase E: Package discovery

- `installed.packages()` scans library directories
- `.libPaths()` returns/sets library search paths
- Package `data/` directories become discoverable via `data()`

## What we can skip for now

- `useDynLib` / `.Call()` — native loading is a separate priority
- Vignette building
- LazyData (can source data/*.R instead)
- S4 method registration via NAMESPACE
- Package installation from source/binary

## File structure

- `src/interpreter/packages.rs` — package discovery, loading, namespace creation
- `src/interpreter/packages/description.rs` — DESCRIPTION parser
- `src/interpreter/packages/namespace.rs` — NAMESPACE parser
- `src/interpreter/packages/search_path.rs` — search path management

## First deliverable

`library(R6)` works — R6 is a pure-R package with no native code, simple
NAMESPACE, and is used by 33 packages in the corpus. If R6 loads and its
classes work, the package runtime is viable.
