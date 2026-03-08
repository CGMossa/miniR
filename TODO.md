# TODO — Stubs & Incomplete Implementations

Everything listed here currently returns NULL, does nothing, or has a simplified implementation. Checked items are done.

## Interpreter Stubs (src/interpreter/mod.rs)

- [ ] Complex numbers — parsed but treated as doubles
- [ ] `..1`, `..2` etc. — parsed but return NULL
- [ ] Formula (`~`) — parsed, binary and unary both return NULL
- [ ] S3 dispatch — `UseMethod()` returns NULL
- [ ] `NextMethod()` — noop

## Builtin Stubs — Core Language

- [ ] `missing(x)` — check if argument was supplied
- [ ] `match.arg(arg, choices)` — match argument to list of choices
- [ ] `switch(expr, ...)` — multi-way branch
- [ ] `do.call(fn, args)` — call function with arg list (partial impl exists)
- [ ] `on.exit(expr)` — register cleanup on function exit
- [ ] `Recall(...)` — recursive self-call
- [ ] `sys.call()` — return current call
- [ ] `sys.frame()` / `sys.frames()` / `sys.nframe()` / `sys.parents()` / `sys.function()` / `sys.on.exit()` — call stack introspection
- [ ] `nargs()` — number of arguments to current function
- [ ] `args(fn)` — formal arguments of function
- [ ] `formals(fn)` — get/set formal argument list
- [ ] `body(fn)` — get/set function body
- [ ] `call(name, ...)` — construct function call
- [ ] `expression(...)` — construct expression object

## Builtin Stubs — Attributes & OOP

- [ ] `attr(x, which)` — get/set attribute
- [ ] `attributes(x)` — get/set all attributes
- [ ] `structure(x, ...)` — set attributes inline
- [ ] `class(x)` — get/set class (partial: returns type name but not real class)
- [ ] `inherits(x, what)` — check class membership
- [ ] `UseMethod(generic)` — S3 method dispatch
- [ ] `NextMethod()` — call next S3 method

## Builtin Stubs — Data Structures

- [ ] `matrix(data, nrow, ncol, byrow)` — create matrix
- [ ] `array(data, dim)` — create array
- [ ] `data.frame(...)` — create data frame
- [ ] `factor(x, levels, labels)` — create factor
- [ ] `levels(x)` / `nlevels(x)` — factor levels
- [ ] `dim(x)` — get/set dimensions
- [ ] `nrow(x)` / `ncol(x)` / `NROW(x)` / `NCOL(x)` — row/column count
- [ ] `t(x)` — transpose
- [ ] `rbind(...)` / `cbind(...)` — row/column bind
- [ ] `diag(x)` — diagonal matrix
- [ ] `crossprod(x)` / `tcrossprod(x)` — cross products
- [ ] `table(...)` / `tabulate(x)` — contingency table

## Builtin Stubs — Apply Family

- [ ] `sapply(X, FUN)` — simplified apply (stub, dispatched to interpreter)
- [ ] `lapply(X, FUN)` — list apply (stub, dispatched to interpreter)
- [ ] `vapply(X, FUN, FUN.VALUE)` — typed apply
- [ ] `apply(X, MARGIN, FUN)` — matrix apply
- [ ] `mapply(FUN, ...)` — multivariate apply
- [ ] `tapply(X, INDEX, FUN)` — table apply
- [ ] `by(data, INDICES, FUN)` — group apply
- [ ] `Vectorize(FUN)` — vectorize a function
- [ ] `Reduce(f, x)` — fold/reduce
- [ ] `Filter(f, x)` — filter elements
- [ ] `Map(f, ...)` — map over multiple lists

## Builtin Stubs — Math & Statistics

- [ ] `pmin(...)` / `pmax(...)` — parallel min/max
- [ ] `norm(x)` — matrix norm
- [ ] `solve(a, b)` — solve linear system
- [ ] `qr(x)` — QR decomposition
- [ ] `svd(x)` — singular value decomposition
- [ ] `eigen(x)` — eigenvalues
- [ ] `det(x)` — determinant
- [ ] `chol(x)` — Cholesky decomposition
- [ ] `complex(...)` — create complex number
- [ ] `cumall(x)` / `cumany(x)` — cumulative logical

## Builtin Stubs — Random Numbers

- [ ] `runif(n, min, max)` — uniform random
- [ ] `rnorm(n, mean, sd)` — normal random
- [ ] `rbinom(n, size, prob)` — binomial random
- [ ] `set.seed(seed)` — registered but may not work

## Builtin Stubs — String & Regex

- [ ] `regexpr(pattern, text)` — regex match positions
- [ ] `gregexpr(pattern, text)` — global regex match
- [ ] `regmatches(x, m)` — extract regex matches
- [ ] `regexec(pattern, text)` — regex match with groups
- [ ] `glob2rx(pattern)` — glob to regex conversion
- [ ] `charToRaw(x)` / `rawToChar(x)` — raw conversion
- [ ] `raw(length)` / `rawShift(x, n)` — raw vectors

## Builtin Stubs — Metaprogramming

- [ ] `eval(expr, envir)` — evaluate expression
- [ ] `evalq(expr, envir)` — evaluate quoted expression
- [ ] `parse(text=)` — parse string to expression
- [ ] `quote(expr)` — return unevaluated expression
- [ ] `substitute(expr, env)` — substitute in expression
- [ ] `bquote(expr)` — partial substitution
- [ ] `deparse(expr)` — expression to string (partial impl)

## Builtin Stubs — Environments

- [ ] `environment(fun)` — get closure environment
- [ ] `new.env(parent)` — create environment
- [ ] `globalenv()` / `baseenv()` / `emptyenv()` — special environments
- [ ] `parent.env(env)` — parent environment
- [ ] `parent.frame(n)` — calling frame
- [ ] `exists(name, envir)` — check binding exists

## Builtin Stubs — Error Handling

- [ ] `tryCatch(expr, error, warning)` — structured error handling
- [ ] `try(expr)` — simple error handling
- [ ] `withCallingHandlers(expr, ...)` — condition handlers
- [ ] `conditionMessage(c)` / `conditionCall(c)` — condition accessors
- [ ] `simpleError(msg)` / `simpleWarning(msg)` / `simpleMessage(msg)` — condition constructors

## Builtin Stubs — File I/O

- [ ] `readLines(con)` — read text lines
- [ ] `writeLines(text, con)` — write text lines
- [ ] `readRDS(file)` / `saveRDS(object, file)` — R serialization
- [ ] `load(file)` / `save(..., file)` — workspace I/O
- [ ] `scan(file, ...)` — read data
- [ ] `source(file)` — execute R file
- [ ] `file.exists(path)` — check file exists
- [ ] `file.copy(from, to)` / `file.create(path)` / `file.remove(path)` / `file.rename(from, to)` — file ops
- [ ] `file.info(path)` / `file.size(path)` — file metadata
- [ ] `dir(path)` / `dir.create(path)` / `dir.exists(path)` / `list.files(path)` — directory ops
- [ ] `tempfile()` / `tempdir()` — temp paths
- [ ] `url(...)` / `open(con)` / `close(con)` / `connection(...)` — connections
- [ ] `unlink(path)` — delete files

## Builtin Stubs — System

- [ ] `system(command)` / `system2(command)` — run shell command
- [ ] `Sys.setenv(...)` — set environment variable
- [ ] `Sys.glob(paths)` — glob expansion
- [ ] `Sys.which(names)` — find executables
- [ ] `normalizePath(path)` / `path.expand(path)` — path normalization
- [ ] `setwd(dir)` — change working directory
- [ ] `install.packages(pkgs)` / `installed.packages()` — package management
- [ ] `require(pkg)` / `library(pkg)` / `loadNamespace(pkg)` / `requireNamespace(pkg)` — package loading (stub prints warning)

## Builtin Stubs — Bitwise

- [ ] `bitwAnd(a, b)` / `bitwOr(a, b)` / `bitwXor(a, b)` — bitwise ops
- [ ] `bitwNot(a)` — bitwise NOT
- [ ] `bitwShiftL(a, n)` / `bitwShiftR(a, n)` — bit shifts

## Builtin Stubs — Misc

- [ ] `reg.finalizer(e, f)` — register finalizer
- [ ] `R.Version()` — version info (stub returns list)

## Missing Builtins (discovered from tests/)

These builtins are needed to pass the R test suite in `tests/`.

### Core / Reflection

- [ ] `dput(x)` — output R representation of object
- [ ] `ls()` — list objects in environment
- [ ] `class<-` — replacement function for setting class
- [ ] `names<-` — replacement function for setting names
- [ ] `dimnames(x)` / `dimnames<-` — get/set dimension names
- [ ] `unname(x)` — remove names
- [ ] `.Platform` — platform info list
- [ ] `capabilities()` — R capabilities
- [ ] `sessionInfo()` — session info
- [ ] `l10n_info()` — localization info
- [ ] `Sys.info()` — system info
- [ ] `Sys.timezone()` — current timezone
- [ ] `suppressWarnings(expr)` — suppress warnings
- [ ] `withVisible(expr)` — evaluate with visibility flag
- [ ] `as.environment(x)` — coerce to environment

### Numeric / Math

- [ ] `outer(X, Y, FUN)` — outer product
- [ ] `Re(z)` / `Im(z)` / `Mod(z)` / `Arg(z)` / `Conj(z)` — complex accessors
- [ ] `signif(x, digits)` — significant digits
- [ ] `lower.tri(x)` / `upper.tri(x)` — matrix triangular extraction
- [ ] `rhyper(nn, m, n, k)` — hypergeometric random
- [ ] `RNGversion(vstr)` — set RNG version

### Strings / IO

- [ ] `intToUtf8(x)` / `utf8ToInt(x)` — UTF-8 ↔ integer
- [ ] `read.table(file)` / `write.table(x, file)` — tabular I/O

### Date/Time

- [ ] `as.POSIXct(x)` / `as.POSIXlt(x)` — datetime constructors
- [ ] `ISOdate(...)` / `ISOdatetime(...)` — ISO datetime
- [ ] `strptime(x, format)` / `strftime(x, format)` — date formatting
- [ ] `format.POSIXlt(x)` — format dates

### S4 / OOP

- [ ] `setClass(Class, ...)` — define S4 class
- [ ] `setMethod(f, ...)` — define S4 method
- [ ] `setGeneric(name, ...)` — define S4 generic

### Packages / Namespaces

- [ ] `tools::*` — tools package namespace
- [ ] `.packages()` — list attached packages
- [ ] `system.file(...)` — find package files
- [ ] `demo(topic)` — run demo

### Graphics (stubs)

- [ ] `pdf(...)` / `dev.off()` — PDF graphics device
- [ ] `windows(...)` — Windows graphics device

### Statistics (stubs)

- [ ] `lm(formula, data)` — linear model
- [ ] `aov(formula)` — analysis of variance
- [ ] `nls(formula, data)` — nonlinear least squares
- [ ] `ts(data, start, frequency)` — time series

## Module Refactoring

- [ ] Ensure all modules use `foo.rs` + `foo/` style, not `foo/mod.rs`

## Misc

- [ ] rename newr to minir.
- [x] add linkme.
- [ ] plan an r package builder
- [ ] add typst conversion of Rdocumentation and produce the manual.
- [x] decouple builtins with proc-macros (interpreter_builtin, pre_eval_builtin, noop_builtin)
