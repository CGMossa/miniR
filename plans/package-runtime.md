# Package and Namespace Runtime

**Status (2026-04-01): Phases A–D complete.** 131/260 CRAN packages load (50%+).
The package runtime is functional for the majority of pure-R and many native packages.
Remaining blockers are native compilation failures (missing system deps) and S7.

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

### Phase A: DESCRIPTION + NAMESPACE parsing — DONE

- DESCRIPTION parser: Package, Version, Depends, Imports, Suggests, LinkingTo, Collate
- NAMESPACE parser: export(), exportPattern(), import(), importFrom(),
  S3method(), useDynLib(), exportClasses(), exportMethods()

### Phase B: Namespace environments — DONE

- Each package gets a namespace env with parent = base env
- R/*.R files sourced in Collate order (from DESCRIPTION) or alphabetically
- Imports resolved: importFrom(rlang, sym) binds sym in the namespace
- Exports create a package env that's a filtered view of the namespace

### Phase C: Search path and library() — DONE

- `library(pkg)` triggers the full loading sequence
- Package env attached to search path (between global and base)
- `::` and `:::` resolve against package/namespace envs
- `require()` returns FALSE instead of erroring if package not found
- `character.only=TRUE` evaluates argument (not just literal TRUE)
- Base packages (base, stats, utils, etc.) treated as built-in no-ops

### Phase D: Hooks and S3 registration — DONE

- `.onLoad(libname, pkgname)` called after sourcing R/
- `.onAttach(libname, pkgname)` called after attaching
- S3 methods declared in NAMESPACE registered in interpreter

### Phase E: Package discovery — MOSTLY DONE

- `installed.packages()` scans library directories
- `.libPaths()` returns/sets library search paths
- `packageDescription()` works for installed and base packages
- `data/` directories: basic support via sysdata.rda loading

## Remaining gaps

- S7 class system — blocks ggplot2 (`class_function`)
- Native compilation failures: fs (libuv), ps (config.h), stringi (ICU),
  openssl, timechange, Matrix (SuiteSparse) — system dep issues
- `system.file()` for base packages returns empty
- LazyData not fully supported (sysdata.rda works, lazy-loading doesn't)

## File structure

- `src/interpreter/packages/loader.rs` — package discovery, loading, namespace creation
- `src/interpreter/packages/description.rs` — DESCRIPTION parser
- `src/interpreter/packages/namespace.rs` — NAMESPACE parser

## Achieved

`library(dplyr)` works — loads rlang, vctrs, tibble, pillar, lifecycle, cli,
and all their transitive dependencies. 131/260 CRAN packages load (50%+).
