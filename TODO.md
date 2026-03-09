# TODO — Stubs & Incomplete Implementations

Everything listed here currently returns NULL, does nothing, or has a simplified implementation.

Items marked 🔧 need no new dependencies (pure Rust / std / already-vendored crates).

## Interpreter Stubs (src/interpreter/mod.rs)

- [ ] Complex numbers — parsed but treated as doubles (see plans/num-complex.md)
- [ ] `..1`, `..2` etc. — parsed but return NULL
- [ ] Formula (`~`) — parsed, binary and unary both return NULL

## Builtin Stubs — Core Language

- [ ] `missing(x)` — registered but always returns FALSE (see plans/call-stack.md)
- [ ] `on.exit(expr)` — register cleanup on function exit (see plans/call-stack.md)
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
- [ ] `complex(...)` — create complex number (see plans/num-complex.md)

## Builtin Stubs — Random Numbers

- [ ] `runif(n, min, max)` — uniform random (see plans/rng-state.md)
- [ ] `rnorm(n, mean, sd)` — normal random (see plans/rng-state.md)
- [ ] `rbinom(n, size, prob)` — binomial random (see plans/rng-state.md)
- [ ] `set.seed(seed)` — registered but returns NULL (see plans/rng-state.md)
- [ ] `sample(x, size, replace, prob)` — random sampling (see plans/rng-state.md)

## Builtin Stubs — String & Regex

- [ ] 🔧 `raw(length)` / `rawShift(x, n)` — raw vectors

## Builtin Stubs — Environments

- [ ] `parent.frame(n)` — calling frame (see plans/call-stack.md)
- [x] 🔧 `as.environment(x)` — coerce to environment

## Builtin Stubs — Error Handling

- [ ] `withCallingHandlers(expr, ...)` — condition handlers (see plans/conditions.md)
- [ ] `conditionMessage(c)` / `conditionCall(c)` — condition accessors (see plans/conditions.md)
- [ ] `simpleError(msg)` / `simpleWarning(msg)` / `simpleMessage(msg)` — condition constructors (see plans/conditions.md)
- [ ] `suppressWarnings(expr)` / `suppressMessages(expr)` — suppress conditions (see plans/conditions.md)
- [x] 🔧 `withVisible(expr)` — evaluate with visibility flag

## Builtin Stubs — File I/O

- [ ] `readRDS(file)` / `saveRDS(object, file)` — R serialization (see plans/serde.md, plans/parquet.md)
- [ ] `load(file)` / `save(..., file)` — workspace I/O (see plans/serde.md)
- [x] 🔧 `scan(file, ...)` — read data
- [x] 🔧 `file.info(path)` — file metadata
- [ ] `tempfile()` / `tempdir()` — rewrite with temp-dir crate for session-scoped cleanup (see plans/temp-dir.md)
- [ ] `url(...)` / `open(con)` / `close(con)` / `connection(...)` — connections
- [x] 🔧 `read.table(file)` / `write.table(x, file)` — tabular I/O
- [ ] `read.parquet(file)` / `write.parquet(df, file)` — Parquet columnar I/O (see plans/parquet.md)

## Builtin Stubs — System

- [ ] `Sys.glob(paths)` — glob expansion (see plans/globset.md)
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
