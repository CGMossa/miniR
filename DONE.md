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

## Attributes & OOP

- `attr(x, which)` — get/set attribute
- `attributes(x)` — get/set all attributes
- `structure(x, ...)` — set attributes inline
- `class(x)` — get/set class (with attribute support)
- `class<-` — replacement function for setting class
- `names<-` — replacement function for setting names
- `inherits(x, what)` — check class membership

## Data Structures

- Basic `data.frame(...)` constructor — creates a classed list with `names` and `row.names`
- `matrix(data, nrow, ncol, byrow)` — create matrix
- `dim(x)` / `dim<-` — get/set dimensions
- `nrow(x)` / `ncol(x)` / `NROW(x)` / `NCOL(x)` — row/column count
- `t(x)` — transpose
- `crossprod(x)` / `tcrossprod(x)` — cross products (via ndarray)
- `diag(x)` — extract diagonal, create diagonal/identity matrix

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
- `lower.tri(x)` / `upper.tri(x)` — triangular matrix extraction

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
- `sys.function()` / `sys.frame()` / `sys.frames()` / `sys.parents()` / `sys.on.exit()` — call-stack introspection
- `sys.nframe()` / `nargs()` / `parent.frame()` — stack depth and caller lookup

## Environments

- `environment(fun)` — get closure environment
- `new.env(parent)` — create environment
- `globalenv()` / `baseenv()` / `emptyenv()` — special environments
- `parent.env(env)` — parent environment
- `environmentName(env)` — environment name
- `is.environment(x)` — environment type check
- `ls()` / `objects()` — list bindings
- `exists(name, envir)` — check binding exists

## Error Handling

- `tryCatch(expr, error, warning)` — structured error handling
- `try(expr)` — simple error handling

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
- `Sys.setenv(...)` — set environment variable
- `Sys.which(names)` — find executables
- `Sys.info()` — system info
- `Sys.timezone()` — current timezone
- `setwd(dir)` — change working directory
- `.Platform` — platform info list
- `capabilities()` — R capabilities
- `sessionInfo()` — session info
- `l10n_info()` — localization info
- `R.Version()` — version info

## Architecture

- add linkme
- decouple builtins with proc-macros (interpreter_builtin, pre_eval_builtin, noop_builtin)
