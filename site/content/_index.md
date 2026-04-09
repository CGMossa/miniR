+++
title = "miniR"
sort_by = "weight"
description = "A reentrant R interpreter written in Rust and aimed at real package compatibility."
+++

## Why miniR exists

miniR is a new R implementation written in Rust for people who care about runtime design, package behavior, and embeddability.

- **Real package code is the target**: the project is measured against a checked-in `cran/` corpus, not only parser fixtures or hand-picked examples.
- **The interpreter is explicitly reentrant**: mutable runtime state belongs on `Interpreter`, so multiple sessions can coexist in one process.
- **Architecture matters**: parser, evaluator, package loading, native code, graphics, and help are kept as readable subsystems rather than collapsing into one giant runtime blob.

## How to use this site

Start with the guide pages if you want the shape of the project:

- `Getting Started` covers builds, feature profiles, and how to point miniR at installed R packages.
- `Interpreter Architecture` explains where parser, evaluator, values, dispatch, package loading, and graphics live in the codebase.
- `CRAN Corpus Compatibility` explains what the headline numbers mean and which missing subsystems still block more packages.

Use the manual when you want the reference view:

- reference pages are generated from the repo `docs/` directory
- pages are ordered intentionally instead of dumped alphabetically
- the focus is on runtime behavior, divergences, and implementation notes that are useful when working on the interpreter itself
