# miniR

A modern R interpreter written in Rust, aimed at running real-world CRAN packages.

miniR is a case study in reimplementing R from scratch — keeping the semantics that work, fixing the ones that don't, and providing better error messages along the way. This is not a drop-in replacement for GNU R; it is a new implementation that respects R's useful semantics while improving on its legacy design decisions.

## Status

Early development. The current tree already has:

- A parser for the full R grammar with custom parse errors and suggestions
- Atomic vectors (logical, integer, double, character, raw, complex), lists, and language objects
- Attributes on vectors and lists, plus matrices, arrays, factors, and a basic `data.frame()`
- Lexical scoping with R's environment chain (base -> global -> local)
- Hundreds of built-in functions across math, strings/regex, I/O, system, factors, conditions, and metaprogramming
- Formula literals (`~`) as classed language objects with `.Environment`
- Call-stack introspection (`sys.*`, `parent.frame()`, `missing()`, `nargs()`, `on.exit()`)
- Partial S3 dispatch, including direct `UseMethod()` and `NextMethod()`
- R's condition system (`tryCatch()`, `withCallingHandlers()`, suppressors, condition constructors)
- CSV/table reading and writing, filesystem/system helpers, and a `reedline` REPL

Major gaps that still need work:

- Connections, serialization, and package loading
- Date/time support, S4, graphics, and broader CRAN runtime compatibility

## Building

```sh
cargo build --release
```

The binary is named `r`:

```sh
# Start the REPL
./target/release/r

# Run a script
./target/release/r script.R

# Evaluate an expression
./target/release/r -e "1 + 1"
```

## Testing

```sh
# Run the test suite
cargo test

# Parse-test against the test corpus
./scripts/parse-test.sh tests/

# Parse-test against top CRAN packages
just update-cran-test-packages
./scripts/parse-test.sh cran/
```

## Goals

1. **Run top CRAN packages** — handle real-world R code, not just toy examples
2. **Well-written code** — clean, idiomatic Rust
3. **Reentrant interpreter** — multiple R interpreters in the same process, no process-global mutable state

## Design Philosophy

We diverge from R behavior when R behavior is absurd. Breaking changes from GNU R are documented in [docs/divergences.md](docs/divergences.md). Error messages are designed to be better than GNU R's — more informative, more specific, with suggestions for how to fix the problem.

## License

MIT
