# Full Feature Verification — 2026-03-18

92 features tested. 82 PASS, 6 PARTIAL, 1 precision edge case.

## Bugs Found

| Bug | Severity | Location |
|---|---|---|
| `sign(0)` returns 1 instead of 0 | Medium | math.rs |
| `substitute()` evaluates arg instead of capturing | High | pre_eval.rs |
| `match.arg()` returns all choices when called with default | Medium | builtins.rs |
| `format(digits=)` ignores digits parameter | Low | interp.rs |
| `rm(x)` requires string, no NSE for bare names | Medium | interp.rs |
| `aggregate()` formula interface fails | Low | interp.rs |

## Formatting Gaps

| Issue | Location |
|---|---|
| Matrix prints as flat vector, not 2D | interp.rs print |
| Factor prints integer codes instead of labels | interp.rs print |
| summary() missing 1st/3rd quartile | interp.rs |
| str() minimal for lists | interp.rs |
| tapply() result missing names | interp.rs |

## Precision

| Issue | Notes |
|---|---|
| pnorm(0) = 0.49999999995 not exactly 0.5 | erfc approximation ~1.5e-7 accuracy |
