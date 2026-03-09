# TODO — Stubs & Incomplete Implementations

Everything listed here currently returns NULL, does nothing, or has a simplified implementation.

Items marked 🔧 need no new dependencies (pure Rust / std / already-vendored crates).

## Interpreter Stubs (src/interpreter/mod.rs)

- [ ] Complex numbers — parsed but treated as doubles
- [ ] `..1`, `..2` etc. — parsed but return NULL
- [ ] Formula (`~`) — parsed, binary and unary both return NULL

## Builtin Stubs — Core Language

- [ ] `missing(x)` — registered but always returns FALSE (needs call-frame arg tracking)
- [ ] `on.exit(expr)` — register cleanup on function exit
- [ ] 🔧 `Recall(...)` — recursive self-call
- [ ] `sys.frame()` / `sys.frames()` / `sys.parents()` / `sys.function()` / `sys.on.exit()` — call stack introspection
- [ ] 🔧 `args(fn)` — formal arguments of function
- [ ] 🔧 `formals(fn)` — get/set formal argument list
- [ ] 🔧 `body(fn)` — get/set function body
- [ ] 🔧 `call(name, ...)` — construct function call
- [ ] 🔧 `expression(...)` — construct expression object

## Builtin Stubs — Data Structures

- [ ] 🔧 `array(data, dim)` — create array
- [ ] `data.frame(...)` — create data frame (see plans/polars-dataframe.md)
- [ ] 🔧 `factor(x, levels, labels)` — create factor
- [ ] 🔧 `levels(x)` / `nlevels(x)` — factor levels
- [ ] 🔧 `rbind(...)` / `cbind(...)` — row/column bind
- [ ] 🔧 `table(...)` / `tabulate(x)` — contingency table
- [ ] 🔧 `dimnames(x)` / `dimnames<-` — get/set dimension names
- [ ] 🔧 `unname(x)` — remove names

## Builtin Stubs — Apply Family

- [ ] 🔧 `apply(X, MARGIN, FUN)` — matrix apply
- [ ] 🔧 `mapply(FUN, ...)` — multivariate apply
- [ ] 🔧 `tapply(X, INDEX, FUN)` — table apply
- [ ] 🔧 `by(data, INDICES, FUN)` — group apply

## Builtin Stubs — Math & Statistics

- [ ] 🔧 `norm(x)` — matrix norm
- [ ] 🔧 `solve(a, b)` — solve linear system (ndarray)
- [ ] 🔧 `outer(X, Y, FUN)` — outer product
- [ ] `qr(x)` — QR decomposition (needs linalg dep)
- [ ] `svd(x)` — singular value decomposition (needs linalg dep)
- [ ] `eigen(x)` — eigenvalues (needs linalg dep)
- [ ] `det(x)` — determinant (needs linalg dep)
- [ ] `chol(x)` — Cholesky decomposition (needs linalg dep)
- [ ] `complex(...)` — create complex number

## Builtin Stubs — Random Numbers

- [ ] 🔧 `runif(n, min, max)` — uniform random (std rand or fastrand)
- [ ] 🔧 `rnorm(n, mean, sd)` — normal random (Box-Muller)
- [ ] 🔧 `rbinom(n, size, prob)` — binomial random
- [ ] 🔧 `set.seed(seed)` — registered but returns NULL (needs RNG state)

## Builtin Stubs — String & Regex

- [ ] 🔧 `raw(length)` / `rawShift(x, n)` — raw vectors

## Builtin Stubs — Environments

- [ ] `parent.frame(n)` — calling frame (needs call stack)
- [ ] 🔧 `as.environment(x)` — coerce to environment

## Builtin Stubs — Error Handling

- [ ] 🔧 `withCallingHandlers(expr, ...)` — condition handlers
- [ ] 🔧 `conditionMessage(c)` / `conditionCall(c)` — condition accessors
- [ ] 🔧 `simpleError(msg)` / `simpleWarning(msg)` / `simpleMessage(msg)` — condition constructors
- [ ] 🔧 `suppressWarnings(expr)` — suppress warnings
- [ ] 🔧 `withVisible(expr)` — evaluate with visibility flag

## Builtin Stubs — File I/O

- [ ] `readRDS(file)` / `saveRDS(object, file)` — R serialization (needs format design)
- [ ] `load(file)` / `save(..., file)` — workspace I/O (needs format design)
- [ ] 🔧 `scan(file, ...)` — read data
- [ ] 🔧 `file.info(path)` — file metadata
- [ ] `tempfile()` / `tempdir()` — rewrite with temp-dir crate for session-scoped cleanup (see plans/temp-dir.md)
- [ ] `url(...)` / `open(con)` / `close(con)` / `connection(...)` — connections
- [ ] 🔧 `read.table(file)` / `write.table(x, file)` — tabular I/O

## Builtin Stubs — System

- [ ] 🔧 `Sys.glob(paths)` — glob expansion
- [ ] `install.packages(pkgs)` / `installed.packages()` — package management
- [ ] `require(pkg)` / `library(pkg)` / `loadNamespace(pkg)` / `requireNamespace(pkg)` — package loading (stub prints warning)

## Builtin Stubs — Date/Time

- [ ] `as.POSIXct(x)` / `as.POSIXlt(x)` — datetime constructors (see plans/chrono.md)
- [ ] `ISOdate(...)` / `ISOdatetime(...)` — ISO datetime
- [ ] `strptime(x, format)` / `strftime(x, format)` — date formatting

## Builtin Stubs — S4 / OOP

- [ ] `setClass(Class, ...)` — define S4 class
- [ ] `setMethod(f, ...)` — define S4 method
- [ ] `setGeneric(name, ...)` — define S4 generic

## Builtin Stubs — Graphics (stubs only)

- [ ] `pdf(...)` / `dev.off()` — PDF graphics device
- [ ] `plot(...)` — plotting
- [ ] `lm(formula, data)` — linear model (see plans/ for stats)

## Builtin Stubs — Misc

- [ ] `reg.finalizer(e, f)` — register finalizer

## Module Refactoring

- [ ] Ensure all modules use `foo.rs` + `foo/` style, not `foo/mod.rs`

## Architecture

- [ ] `Language(Box<Expr>)` should have a dedicated `Language` newtype, so the enum variant becomes `Language(Language)` — use derive_more if needed
- [ ] rename newr to minir
- [ ] plan an r package builder
- [ ] add typst conversion of R documentation and produce the manual
