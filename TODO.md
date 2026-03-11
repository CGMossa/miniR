# TODO — Stubs & Incomplete Implementations

Everything listed here currently returns NULL, does nothing, or has a simplified implementation.

Items marked 🔧 need no new dependencies (pure Rust / std / already-vendored crates).

## Interpreter Stubs (src/interpreter/mod.rs)

- [x] 🔧 Complex numbers — full support via num-complex (Vector::Complex, arithmetic, Re/Im/Mod/Arg/Conj)
- [x] 🔧 `..1`, `..2` etc. — element access into `...` list
- [ ] Formula (`~`) — parsed, binary and unary both return NULL

## Builtin Stubs — Core Language

- [ ] `missing(x)` — registered but always returns FALSE (see plans/call-stack.md)
- [x] 🔧 `on.exit(expr)` — register cleanup on function exit
- [x] 🔧 `Recall(...)` — recursive self-call (informative error until call stack exists)
- [ ] `sys.frame()` / `sys.frames()` / `sys.parents()` / `sys.function()` / `sys.on.exit()` — call stack introspection (see plans/call-stack.md)
- [x] 🔧 `args(fn)` — formal arguments of function
- [x] 🔧 `formals(fn)` — get/set formal argument list
- [x] 🔧 `body(fn)` — get/set function body
- [x] 🔧 `call(name, ...)` — construct function call
- [x] 🔧 `expression(...)` — construct expression object

## Builtin Stubs — Data Structures

- [x] 🔧 `array(data, dim)` — create array
- [ ] `data.frame(...)` — create data frame (see plans/polars-dataframe.md)
- [x] 🔧 `factor(x, levels, labels)` — create factor
- [x] 🔧 `levels(x)` / `nlevels(x)` — factor levels
- [x] 🔧 `rbind(...)` / `cbind(...)` — row/column bind
- [x] 🔧 `table(...)` / `tabulate(x)` — contingency table
- [x] 🔧 `dimnames(x)` / `dimnames<-` — get/set dimension names
- [x] 🔧 `unname(x)` — remove names

## Builtin Stubs — Apply Family

- [x] 🔧 `apply(X, MARGIN, FUN)` — matrix apply
- [x] 🔧 `mapply(FUN, ...)` — multivariate apply
- [x] 🔧 `tapply(X, INDEX, FUN)` — table apply
- [x] 🔧 `by(data, INDICES, FUN)` — group apply

## Builtin Stubs — Math & Statistics

- [x] 🔧 `norm(x)` — matrix norm
- [x] 🔧 `solve(a, b)` — solve linear system (ndarray)
- [x] 🔧 `outer(X, Y, FUN)` — outer product
- [ ] `qr(x)` — QR decomposition (see plans/nalgebra.md)
- [ ] `svd(x)` — singular value decomposition (see plans/nalgebra.md)
- [ ] `eigen(x)` — eigenvalues (see plans/nalgebra.md)
- [ ] `det(x)` — determinant (see plans/nalgebra.md)
- [ ] `chol(x)` — Cholesky decomposition (see plans/nalgebra.md)
- [x] 🔧 `complex(...)` — create complex number via num-complex

## Builtin Stubs — Random Numbers

- [x] 🔧 `runif(n, min, max)` — uniform random
- [x] 🔧 `rnorm(n, mean, sd)` — normal random
- [x] 🔧 `rbinom(n, size, prob)` — binomial random
- [x] 🔧 `rpois(n, lambda)` — Poisson random
- [x] 🔧 `rexp(n, rate)` — exponential random
- [x] 🔧 `rgamma(n, shape, rate)` — gamma random
- [x] 🔧 `rbeta(n, shape1, shape2)` — beta random
- [x] 🔧 `rcauchy(n, location, scale)` — Cauchy random
- [x] 🔧 `rchisq(n, df)` — chi-squared random
- [x] 🔧 `rt(n, df)` — Student's t random
- [x] 🔧 `rf(n, df1, df2)` — F random
- [x] 🔧 `rgeom(n, prob)` — geometric random
- [x] 🔧 `rhyper(nn, m, n, k)` — hypergeometric random
- [x] 🔧 `rweibull(n, shape, scale)` — Weibull random
- [x] 🔧 `rlnorm(n, meanlog, sdlog)` — log-normal random
- [x] 🔧 `set.seed(seed)` — seed RNG for reproducibility
- [x] 🔧 `sample(x, size, replace, prob)` — random sampling

## Builtin Stubs — String & Regex

- [ ] 🔧 `raw(length)` / `rawShift(x, n)` — raw vectors

## Builtin Stubs — Environments

- [ ] `parent.frame(n)` — calling frame (see plans/call-stack.md)
- [x] 🔧 `as.environment(x)` — coerce to environment

## Builtin Stubs — Error Handling

- [x] 🔧 `withCallingHandlers(expr, ...)` — condition handlers (see plans/conditions.md)
- [x] 🔧 `conditionMessage(c)` / `conditionCall(c)` — condition accessors (see plans/conditions.md)
- [x] 🔧 `simpleError(msg)` / `simpleWarning(msg)` / `simpleMessage(msg)` / `simpleCondition(msg)` — condition constructors (see plans/conditions.md)
- [x] 🔧 `suppressWarnings(expr)` / `suppressMessages(expr)` — suppress conditions (see plans/conditions.md)
- [x] 🔧 `invokeRestart(name)` — invoke a restart (muffleWarning, muffleMessage)
- [x] 🔧 `withVisible(expr)` — evaluate with visibility flag

## Builtin Stubs — File I/O

- [ ] `readRDS(file)` / `saveRDS(object, file)` — R serialization (see plans/serde.md, plans/parquet.md)
- [ ] `load(file)` / `save(..., file)` — workspace I/O (see plans/serde.md)
- [x] 🔧 `scan(file, ...)` — read data
- [x] 🔧 `file.info(path)` — file metadata
- [x] 🔧 `tempfile()` / `tempdir()` — session-scoped with auto-cleanup via temp-dir crate
- [ ] `url(...)` / `open(con)` / `close(con)` / `connection(...)` — connections
- [x] 🔧 `read.table(file)` / `write.table(x, file)` — tabular I/O
- [ ] `read.parquet(file)` / `write.parquet(df, file)` — Parquet columnar I/O (see plans/parquet.md)

## Builtin Stubs — System

- [x] 🔧 `Sys.glob(paths)` — glob expansion via glob crate
- [ ] `install.packages(pkgs)` / `installed.packages()` — package management
- [ ] `require(pkg)` / `library(pkg)` / `loadNamespace(pkg)` / `requireNamespace(pkg)` — package loading (stub prints warning)

## Builtin Stubs — Date/Time

- [ ] `as.POSIXct(x)` / `as.POSIXlt(x)` — datetime constructors (see plans/chrono.md, plans/jiff.md)
- [ ] `ISOdate(...)` / `ISOdatetime(...)` — ISO datetime (see plans/chrono.md, plans/jiff.md)
- [ ] `strptime(x, format)` / `strftime(x, format)` — date formatting (see plans/chrono.md, plans/jiff.md)

## Builtin Stubs — S4 / OOP

- [ ] `setClass(Class, ...)` — define S4 class
- [ ] `setMethod(f, ...)` — define S4 method
- [ ] `setGeneric(name, ...)` — define S4 generic

## Builtin Stubs — Graphics (stubs only)

- [ ] `pdf(...)` / `dev.off()` — PDF graphics device
- [ ] `plot(...)` — plotting
- [ ] `lm(formula, data)` — linear model (needs stats plan, depends on formula + data.frame)

## Builtin Stubs — Misc

- [ ] `reg.finalizer(e, f)` — register finalizer

## Module Refactoring

- [ ] Ensure all modules use `foo.rs` + `foo/` style, not `foo/mod.rs`

## Architecture

- [x] `Language(Box<Expr>)` should have a dedicated `Language` newtype, so the enum variant becomes `Language(Language)` — use derive_more if needed
- [ ] rename newr to minir
- [ ] plan an r package builder
- [ ] add typst conversion of R documentation and produce the manual
- [ ] Arrow backend for vector types — replace `Vec<Option<T>>` with validity bitmap + contiguous buffer (see plans/arrow-backend.md)
- [ ] Per-module error types — replace centralized `RError` with module-specific errors using derive_more (see plans/module-error-types.md)
- [ ] Feature-gate the IO module for sandboxed/WASM environments (see plans/io-feature-gate.md)

## Developer Experience

- [ ] Add tokei for file size tracking and refactoring detection (see plans/tokei-file-tracking.md)
- [ ] Vendor patch system for modifying vendored dependencies (see plans/vendor-patches.md)

## REPL

- [ ] Implement reedline features: persistent history, validator, highlighting, hints, completions (see plans/reedline-features.md)

## Quick Wins

- [ ] 🔧 Nice error message for `..0` — R uses 1-based indexing for `...` args (see plans/dotdot-zero-error.md)
- [ ] 🔧 Optimize `sort(unique())` with BTreeSet / add `sort_unique()` builtin (see plans/sort-unique-optimization.md)
