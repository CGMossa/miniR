# Full Feature Verification — 2026-03-18

92 features tested. 82 PASS, 6 PARTIAL, 1 precision edge case.

## Bugs Found

| Bug | Severity | Status |
|---|---|---|
| `sign(0)` returns 1 instead of 0 | Medium | **FIXED** |
| `substitute()` evaluates arg instead of capturing | High | **FIXED** (promises) |
| `match.arg()` returns all choices when called with default | Medium | **FIXED** |
| `format(digits=)` ignores digits parameter | Low | Open |
| `rm(x)` requires string, no NSE for bare names | Medium | Open |
| `aggregate()` formula interface fails | Low | Open |

## Formatting Gaps

| Issue | Status |
|---|---|
| Matrix prints as flat vector, not 2D | **FIXED** |
| Factor prints integer codes instead of labels | **FIXED** |
| summary() missing 1st/3rd quartile | Open |
| str() minimal for lists | Open |
| tapply() result missing names | Open |

## Precision

| Issue | Status |
|---|---|
| pnorm(0) = 0.49999999995 not exactly 0.5 | **FIXED** (libm::erfc) |
