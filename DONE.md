# DONE — Completed Implementations

Items moved here from TODO.md once their core user-facing behavior stops being a stub.
If a feature still has important semantic gaps, keep those gaps in `TODO.md` or the relevant plan doc.

## Core Language

- `match.arg(arg, choices)` — match argument to list of choices
- `switch(expr, ...)` — multi-way branch
- `do.call(fn, args)` — call function with arg list
- `as.vector(x)` — strip vector/list attributes in the common path
- `missing(x)` — supplied/defaulted argument detection via call frames
- `on.exit(expr)` — register exit handlers on the active call frame
- Formula literals (`~`) — classed language objects with `.Environment`
- `UseMethod(generic, object)` / `NextMethod()` — S3 dispatch with method-visible call frames
- Three-pass argument matching — exact name → partial prefix → positional, with unused-argument and ambiguous-partial errors
- `<<-` super-assignment — creates bindings in global env (not base)
- Builtin `min_args` enforcement — dispatcher checks both min and max arity
- `system.time(expr)` — pre-eval builtin that times the unevaluated expression
- `NULL` visibility — visible at top level unless `invisible()`

## Attributes & OOP

- `attr(x, which)` — get/set attribute
- `attributes(x)` — get/set all attributes
- `structure(x, ...)` — set attributes inline
- `class(x)` — get/set class (with attribute support)
- `class<-` — replacement function for setting class
- `names<-` — replacement function for setting names
- `inherits(x, what)` — check class membership
- `print()` / `format()` — S3 generics dispatching to print.Date, format.POSIXct, etc.

## Type Stability and Attributes

- Type-preserving indexing — matrix subsetting preserves integer/character type
- Type-preserving assignment — `x[1] <- 2L` on integer stays integer
- Attribute preservation through assignment — `m[1] <- 9L` preserves dim/dimnames
- Attribute propagation through arithmetic — `m + 1` preserves dim/names
- Logical index recycling — `x[c(TRUE, FALSE)]` recycles mask to target length
- Mixed positive/negative index validation — `x[c(-1, 2)]` errors
- Matrix dimname indexing — `m["r1", "c1"]` resolves character indices against dimnames
- Data frame row-name preservation — `df[2:3, ]` keeps selected row names
- `c()` name preservation — `c(a=1, b=2)` produces named vector
- `matrix()` type preservation — `matrix(1:4, 2, 2)` stays integer

## Data Structures

- `data.frame(...)` — common constructor path handles recycling, `row.names`, list/matrix expansion, and `stringsAsFactors`
- `matrix(data, nrow, ncol, byrow, dimnames)` — create matrix with optional dimension names, type-preserving
- `row.names(x)` / `colnames(x)` / `rownames<-` / `colnames<-` — retrieve and update row and column labels on data frames and matrices
- `dimnames(x)` / data-frame-aware `dimnames<-` — keep matrix and data-frame labels visible through the shared dimnames interface
- `dim(x)` / `dim<-` — get/set dimensions
- `nrow(x)` / `ncol(x)` / `NROW(x)` / `NCOL(x)` — row/column count
- `t(x)` — type-preserving transpose with dimname swapping
- `crossprod(x)` / `tcrossprod(x)` — cross products with matrix class and common dimname propagation
- `diag(x)` — extract diagonal, create diagonal/identity matrix with matrix-class outputs

## Apply Family

- `sapply(X, FUN)` — simplified apply
- `lapply(X, FUN)` — list apply
- `vapply(X, FUN, FUN.VALUE)` — typed apply
- `Vectorize(FUN)` — vectorize a function
- `Reduce(f, x)` — fold/reduce
- `Filter(f, x)` — filter elements
- `Map(f, ...)` — map over multiple lists

## Math & Statistics

- `pmin(...)` / `pmax(...)` — parallel min/max with recycling and NA propagation
- `signif(x, digits)` — round to significant digits
- `cumall(x)` / `cumany(x)` — cumulative logical AND/OR
- `lower.tri(x)` / `upper.tri(x)` — triangular matrix extraction as logical matrices
- `det(x)` — matrix determinant via Gaussian elimination with partial pivoting
- `chol(x)` — Cholesky decomposition (upper triangular)
- `qr(x)` — QR decomposition via Householder reflections
- `svd(x)` — singular value decomposition via one-sided Jacobi
- `eigen(x)` — eigenvalue decomposition for symmetric matrices via Jacobi
- `solve(a, b)` — linear system solve / matrix inverse
- `norm(x, type)` — matrix norms (1, Inf, Frobenius, max)
- `lm(formula, data)` — linear regression via OLS (simple + multiple)
- `summary()` — S3 generic with summary.lm method
- `coef(x)` — extract model coefficients
- `diff(x, lag)` — lagged differences with configurable lag parameter

## Bitwise

- `bitwAnd(a, b)` / `bitwOr(a, b)` / `bitwXor(a, b)` — bitwise ops
- `bitwNot(a)` — bitwise NOT
- `bitwShiftL(a, n)` / `bitwShiftR(a, n)` — bit shifts

## String & Regex

- `intToUtf8(x)` / `utf8ToInt(x)` — UTF-8 integer conversion
- `charToRaw(x)` / `rawToChar(x)` — raw/byte conversion
- `glob2rx(pattern)` — glob to regex conversion
- `regexec(pattern, text)` — regex match with capture groups
- `regexpr(pattern, text)` — regex match positions
- `gregexpr(pattern, text)` — global regex match
- `regmatches(x, m)` — extract regex matches
- `strsplit(x, split, fixed)` — regex and fixed string splitting
- `sprintf(fmt, ...)` — proper format specifiers (%s, %d, %f, %e, %g with width/precision/flags)
- `trimws(x, which)` — trim whitespace with "both"/"left"/"right" parameter
- `startsWith(x, prefix)` / `endsWith(x, suffix)` — vectorized prefix/suffix testing

## Date/Time

- `Sys.Date()` — current date as Date class
- `Sys.time()` — current time as POSIXct class (jiff)
- `as.Date(x)` — parse/coerce to Date
- `as.POSIXct(x, tz)` — parse/coerce to POSIXct
- `as.POSIXlt(x, tz)` — decompose to POSIXlt list
- `format.Date(x, format)` / `format.POSIXct(x, format)` — date/time formatting
- `strptime(x, format)` / `strftime(x, format)` — parse/format with format strings
- `difftime(t1, t2, units)` — time differences
- `weekdays(x)` / `months(x)` / `quarters(x)` — date component extraction
- `print.Date` / `print.POSIXct` — S3 print methods for dates

## Metaprogramming

- `eval(expr, envir)` — evaluate expression
- `parse(text=)` — parse string to expression
- `quote(expr)` — return unevaluated expression
- `substitute(expr, env)` — substitute in expression
- `evalq(expr, envir)` — evaluate quoted expression
- `bquote(expr)` — partial substitution with `.()` splicing
- `deparse(expr)` — expression to string
- `dput(x)` — output R representation of object
- `sys.call()` / `sys.calls()` — current and active call expressions

## I/O

- `readRDS(file)` / `saveRDS(object, file)` — text-based miniRDS round-trip for common miniR values
- `load(file)` / `save(..., file)` — text-based miniR workspace round-trip for named bindings
- `sys.function()` / `sys.frame()` / `sys.frames()` / `sys.parents()` / `sys.on.exit()` — call-stack introspection
- `sys.nframe()` / `nargs()` / `parent.frame()` — stack depth and caller lookup
- Feature-gated IO module — `#[cfg(feature = "io")]` for sandboxed/WASM builds

## Environments

- `environment(fun)` — get closure environment (interpreter builtin for no-arg case)
- `new.env(parent)` — create environment
- `globalenv()` / `baseenv()` / `emptyenv()` — special environments
- `parent.env(env)` — parent environment
- `environmentName(env)` — environment name
- `is.environment(x)` — environment type check
- `ls()` / `objects()` — list bindings
- `exists(name, envir)` — check binding exists
- Per-interpreter env vars — `Sys.setenv()`/`Sys.getenv()` are interpreter-local
- Per-interpreter working directory — `setwd()`/`getwd()` are interpreter-local

## Error Handling

- `tryCatch(expr, error, warning)` — structured error handling
- `try(expr)` — simple error handling
- `withCallingHandlers(expr, ...)` — non-unwinding condition handlers
- `suppressWarnings(expr)` / `suppressMessages(expr)` — condition suppression
- `simpleError(msg)` / `simpleWarning(msg)` / `simpleMessage(msg)` — condition constructors
- `conditionMessage(c)` / `conditionCall(c)` — condition accessors

## File I/O

- `readLines(con)` — read text lines
- `writeLines(text, con)` — write text lines
- `source(file)` — execute R file
- `file.exists(path)` — check file exists
- `file.copy(from, to)` / `file.create(path)` / `file.remove(path)` / `file.rename(from, to)` — file ops
- `file.size(path)` — file metadata
- `dir(path)` / `dir.create(path)` / `dir.exists(path)` / `list.files(path)` — directory ops
- `tempfile()` / `tempdir()` — session-scoped temp paths via `temp-dir`
- `unlink(path)` — delete files
- `normalizePath(path)` / `path.expand(path)` — path normalization

## System

- `system(command)` / `system2(command)` — run shell command
- `Sys.setenv(...)` — set environment variable (per-interpreter)
- `Sys.getenv(name)` — get environment variable (per-interpreter)
- `Sys.which(names)` — find executables (uses interpreter PATH)
- `Sys.info()` — system info
- `Sys.timezone()` — current timezone
- `Sys.sleep(time)` — sleep for n seconds
- `setwd(dir)` / `getwd()` — working directory (per-interpreter)
- `proc.time()` — elapsed time from interpreter creation
- `.Platform` — platform info list
- `capabilities()` — R capabilities
- `sessionInfo()` — session info
- `l10n_info()` — localization info
- `R.Version()` — version info

## Help System

- `?name` / `help(name)` — displays docs from rustdoc comments on builtins
- `Builtin` trait + `FromArgs` derive — struct-based builtin definition with auto-registration and doc extraction

## Architecture

- Linkme-based builtin registry with unified `BuiltinDescriptor`
- Proc-macro builtin registration (`#[builtin]`, `#[interpreter_builtin]`, `#[pre_eval_builtin]`)
- `BuiltinContext` — all builtins use explicit context, zero TLS in builtin layer
- Module extraction: `ops.rs`, `assignment.rs`, `indexing.rs`, `control_flow.rs`, `call_eval.rs`, `s3.rs`, `arguments.rs`
- `Session` API — library boundary with `eval_source()`, `eval_file()`, `eval_expr()`
- Parser diagnostics split into `diagnostics.rs` with UTF-8-safe context building
- `CallArgs` helper for argument decoding
- `From`/`TryFrom` conversions between R value types
- CI via GitHub Actions (fmt, clippy, test with vendored deps)

## Testing

- `tests/smoke.rs` — end-to-end ops, assignment, indexing, datetime, S3 dispatch
- `tests/reentrancy.rs` — session isolation, nested eval, parallel threads
- `tests/parse_corpus.rs` — all .R files via Session API with regression baseline
- `tests/argument_matching.rs` — three-pass matching conformance
- `tests/lm.rs` — linear regression with 5 tests
- REPL history fallback — gracefully handles unwritable history files

## Distributions (stats)

- `dexp/pexp/qexp` — exponential density/CDF/quantile
- `dgamma/pgamma/qgamma` — gamma distribution
- `dbeta/pbeta/qbeta` — beta distribution
- `dcauchy/pcauchy/qcauchy` — Cauchy distribution
- `dweibull/pweibull/qweibull` — Weibull distribution
- `dlnorm/plnorm/qlnorm` — log-normal distribution
- `dchisq/pchisq/qchisq` — chi-squared distribution
- `dt/pt/qt` — Student's t distribution
- `df/pf/qf` — F distribution
- `dbinom/pbinom/qbinom` — binomial distribution
- `dpois/ppois/qpois` — Poisson distribution
- `dgeom/pgeom/qgeom` — geometric distribution
- `dhyper/phyper/qhyper` — hypergeometric distribution
- `cor(x,y)` / `cov(x,y)` — correlation and covariance
- `weighted.mean(x,w)` — weighted mean
- `scale(x)` — center and scale
- `complete.cases(...)` — detect complete observations
- `na.omit(x)` — remove NAs

## Extended Distributions (miniR extensions)

- `rfrechet`, `rgumbel`, `rinvgauss`, `rpareto`, `rpert`, `rskewnorm`, `rtriangular`, `rzeta`

## Special Math Functions

- `digamma(x)` / `trigamma(x)` — psi functions via libm
- `besselJ(x, nu)` / `besselY(x, nu)` — Bessel functions via libm
- `cbrt(x)` — cube root
- `hypot(x, y)` — overflow-safe Euclidean distance
- `gamma(x)` / `lgamma(x)` / `beta(a,b)` / `lbeta(a,b)` — gamma/beta functions
- `factorial(n)` / `choose(n,k)` / `combn(n,k)` — combinatorics
- `asin/acos/atan/atan2/sinh/cosh/tanh/asinh/acosh/atanh` — trig/hyperbolic
- `expm1(x)` / `log1p(x)` — numerically stable variants

## Collections (miniR extensions)

- HashMap: `hashmap`, `hashmap_set/get/has/remove/keys/values/size/to_list`
- BTreeMap: `btreemap_*` (ordered key-value store)
- HashSet: `hashset_*` with set operations (union/intersect/diff)
- BinaryHeap: `heap_*` (max priority queue)
- VecDeque: `deque_*` (double-ended queue)

## JSON Support

- `fromJSON(txt)` / `toJSON(x)` — jsonlite-compatible JSON ↔ R conversion
- Smart coercion: arrays of objects → data.frames, scalars → vectors

## TOML Support

- `read.toml(file)` / `write.toml(x, file)` — TOML file I/O
- `toml_parse(text)` / `toml_serialize(x)` — TOML string conversion
- Array of tables → data.frame coercion

## Networking

- `make.socket(host, port)` / `read.socket` / `write.socket` / `close.socket` — TCP client
- `download.file(url, destfile)` — HTTP/HTTPS file download (rustls TLS)
- `url("https://...")` — HTTPS connection support

## Cryptographic Hashing

- `digest(x, algo)` — SHA-256/SHA-512 hashing
- `md5(x)` — informative error suggesting sha256

## Rich Diagnostics

- miette-powered parse error rendering with source spans and suggestions
- `View(x)` — tabled-powered data.frame display with column types
- `kable(x)` — markdown table rendering
- `str()` for data.frames — aligned column summary via tabled

## Additional Builtins

- `match.call()` — reconstruct matched call
- `Find/Position/Negate` — functional programming helpers
- `rapply(object, f, how)` — recursive apply
- `cut(x, breaks)` / `findInterval(x, vec)` — binning
- `split(x, f)` / `unsplit(value, f)` — group splitting
- `aggregate(x, by, FUN)` — group aggregation
- `rm()` / `remove()` — remove bindings
- `local(expr)` — evaluate in local environment
- `lockEnvironment` / `lockBinding` — environment locking
- `Encoding(x)` / `enc2utf8` / `enc2native` — encoding stubs
- `strtrim(x, width)` — trim to display width
- `arrayInd(ind, dim)` — linear to array index
- `xor(x, y)` — vectorized exclusive OR
- `URLencode/URLdecode` — percent-encoding
- `casefold(x, upper)` — case conversion
- `encodeString(x, quote)` — string encoding with quoting
- `substr<-` — substring replacement
- `strrep(x, times)` — string repetition
- `pmatch/charmatch` — partial matching
- `formatC/format.pval/prettyNum` — formatting utilities
- `options()/getOption()` — per-interpreter R options
- `.Machine` — machine constants
- `Sys.getpid()` / `Sys.localeconv()` — system info
- `gc()/gcinfo()/debug()/undebug()/browser()` — stubs
- `rank(x, ties.method)` — ranking with tie-breaking
- `aperm(a, perm)` — array permutation

## Dependency Integrations

- `itertools` — join/sorted/unique across codebase
- `indexmap` — ordered attributes (preserves R's attribute order)
- `tabwriter` — elastic tabstop data.frame printing
- `tabled` — rich table rendering (View, kable, str)
- `unicode-width` — display-width-aware alignment
- `unicode-segmentation` — grapheme cluster counting
- `unicase` — case-insensitive string matching
- `signal-hook` — Ctrl-C interrupt handling
- `dirs/walkdir/globset` — platform paths, recursive listing, glob validation
- `bstr` — byte string handling for non-UTF-8 files
- `rayon` — parallel vectorized arithmetic (opt-in)
- `serde_json` — JSON support
- `toml_edit` — TOML support
- `sha2` — cryptographic hashing
- `rustls` — TLS for HTTPS (opt-in)
- `miette` — rich error diagnostics
- `log/env_logger` — logging infrastructure (opt-in)
- `slotmap` — vendored for future arena allocation
- `SmallRng` — faster non-crypto RNG with .Random.seed support

## Feature Gates

- `linalg`, `tables`, `signal`, `tls`, `diagnostics`, `logging`, `parallel`, `arena`, `toml`, `json`, `digest`, `collections`, `random`, `datetime`, `io`, `dirs-support`, `walkdir-support`, `globset-support`
- `cargo build --no-default-features` compiles minimal interpreter

## Namespace Support

- Builtins annotated with namespace (base/stats/utils/collections/net/jsonlite/digest/grDevices/graphics/methods)
- `pkg::name` resolution checks builtin registry
- `?pkg::name` shows namespaced help
