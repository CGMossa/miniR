# Builtin Audit — 2026-03-17

625 builtins across 10 namespaces. 4 agents audited every function.

## Summary Counts

| Status | Count | Description |
|---|---|---|
| REAL | ~540 | Correct or close enough for CRAN |
| PARTIAL | ~55 | Works but missing key behavior |
| STUB | ~30 | Placeholder only (graphics, S4, package mgmt, debugging) |

## Critical Bugs (fix immediately)

| Bug | Location | Impact |
|---|---|---|
| **`ifelse` not vectorized** | builtins.rs:2074 | Uses `as_logical_scalar()` — only reads first element. `ifelse(c(T,F,T), 1:3, 4:6)` returns `1:3` not `c(1,5,3)` |
| **`replace` only works on doubles** | builtins.rs:2258 | Coerces via `to_doubles()`. Character/integer/logical replacement produces NA |
| **`round()` wrong rounding rule** | math.rs:783 | Uses round-half-away-from-zero (Rust default). R uses round-half-to-even (IEEE 754). `round(0.5)` → 1 not 0 |
| **`log()` ignores base arg** | math.rs:80 | `log(100, 10)` returns `log(100)` not 2 |
| **`cumsum/cumprod/cummax/cummin` don't propagate NA** | math.rs:1508-1613 | `cumsum(c(1,NA,3))` → `c(1,NA,3)` not `c(1,NA,NA)` |
| **`var()/sd()/median()/range()` silently drop NAs** | math.rs:1442-1488, 2383 | Should return NA by default, only drop with `na.rm=TRUE` |
| **`is.ordered/is.call/is.symbol/is.name/is.expression/is.pairlist` aliased to `is.null`** | types.rs:18 | All return TRUE only for NULL. Completely wrong |

## Systemic Gaps (many functions affected)

| Gap | Affected | Fix approach |
|---|---|---|
| **All 48 d/p/q distribution functions missing `lower.tail`, `log.p`, `log` params** | stats.rs (all d/p/q*) | Add named arg extraction to each; `lower.tail=FALSE` → 1-p; `log=TRUE` → log(d); `log.p=TRUE` → log(p) |
| **~12 string functions are scalar-only, not vectorized** | strings.rs: substr, strsplit, chartr, basename, dirname, strtoi, glob2rx, URLencode, URLdecode, substr<- | Wrap in per-element loop |
| **`grepl`/`grep` return FALSE for NA inputs** | strings.rs:311,345 | Should return NA |
| **`rep()` missing `each` param** | math.rs:1721 | `rep(1:3, each=2)` → `c(1,1,2,2,3,3)` very common |
| **`sample()` ignores `prob` weights** | random.rs:573 | Accepted but unused |

## Stubs (intentionally deferred)

| Category | Functions | Why deferred |
|---|---|---|
| **Graphics** | plot, points, lines, abline, legend, title, axis, par, pdf, png, svg, dev.off/cur/new | No graphics device yet |
| **S4 OOP** | setClass, setGeneric, setMethod, isVirtualClass, validObject, setValidity, showClass, existsMethod | S4 requires full class registry |
| **Package mgmt** | library, require, loadNamespace, requireNamespace, installed.packages, install.packages, system.file | Package runtime not built |
| **Debugging** | debug, undebug, isdebugged, browser, traceback | Debugger not built |
| **GC** | gc, gcinfo | No GC needed in Rust |
| **invokeRestart** | conditions.rs:343 | Restart mechanism not implemented |
| **makeActiveBinding** | interp.rs:2048 | Calls fun() once, stores result — no re-evaluation on access |

## Partial Implementations (worth fixing)

| Function | Issue | Priority |
|---|---|---|
| `identical` | Uses Debug format comparison — fragile for NaN, envs, closures | Medium |
| `unlist` | Flattens one level only, no recursive=FALSE, bad name generation | Medium |
| `invisible` | Doesn't set invisible flag — auto-printing still occurs | Medium |
| `str` | No recursive list traversal or nested indentation | Low |
| `format` | Default method just uses Display trait, no nsmall/width/big.mark | Low |
| `parse` | Returns single Language, R returns expression vector | Low |
| `substitute` | Doesn't handle for/while/function in substitution | Medium |
| `sort` | Missing na.last parameter | Medium |
| `order` | Missing decreasing + multi-key for the math.rs version | Medium |
| `append` | Always coerces to character | High |
| `diff` | Missing differences parameter | Low |
| `eigen` | Errors on complex eigenvalues instead of returning them | Low |
| `lm` | Missing R², std errors, summary.lm stats table | Low |
| `cor` | Only Pearson, missing Spearman/Kendall | Low |
| `sign` | Returns double for integer input, NaN instead of NA | Low |
| `is.primitive` | Conflated with is.function | Low |
| `is.array` | Only checks class, not dim attribute | Low |
