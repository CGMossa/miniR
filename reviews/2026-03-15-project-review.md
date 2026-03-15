# Project Review

Date: 2026-03-15

This review is based on a read-through of the core interpreter, parser, session, environment, indexing, assignment, and builtin layers, plus targeted runtime repros and project checks.

Checks run:

- `cargo fmt --check` — passed
- `cargo test` — passed
- `cargo clippy --all-targets --all-features -- -D warnings` — failed
- Targeted runtime repros with `target/debug/r -e ...`
- Targeted Rust harnesses against the library API for interpreter-state checks

The core theme is that the current tree already has a lot of functionality, but several of the remaining gaps are not “missing builtin” problems. They are semantic mismatches in the evaluator, indexing, attribute handling, and interpreter state model. Those will block real CRAN code faster than another tranche of leaf builtins.

## Highest-priority fixes

### 1. Public `Interpreter::eval()` still depends on ambient TLS

Affected code:

- `src/interpreter.rs:22-63`
- `src/interpreter.rs:192-196`
- `src/interpreter/builtins/system.rs:402-439`
- Also many `with_interpreter()` call sites in `src/interpreter/builtins/pre_eval.rs`, `src/interpreter/builtins/random.rs`, `src/interpreter/builtins/io.rs`, and `src/interpreter/builtins/conditions.rs`

Why this matters:

- The public evaluator API is not self-contained.
- A caller can invoke `Interpreter::eval()` on interpreter `A`, but builtins that still consult TLS will read state from whatever interpreter happens to be installed in TLS at the time.
- That directly violates the project goal of multiple interpreters coexisting safely in one process.

Confirmed reproduction:

```text
direct=/var/.../t12a99-1
ambient1=/var/.../t12a99-2
ambient2=/var/.../t12a99-3
direct_eq_ambient1=false
ambient1_eq_ambient2=false
```

That output came from evaluating `tempdir()` on the same `Interpreter` instance under different ambient TLS states. The result changed based on ambient state, not on the `Interpreter` value being called.

Suggested fix:

- Stop using `with_interpreter()` from builtin implementations that already have a real interpreter context available.
- Thread `&Interpreter` or `BuiltinContext` through every builtin path, including pre-eval builtins.
- Make `Interpreter::eval()` install `self` for the duration of evaluation, or make raw `Interpreter::eval()` non-public and force callers through `Session`.
- Treat remaining TLS lookups as bugs to be burned down, not as a permanent architecture layer.

Suggested tests:

- A regression test that evaluates `tempdir()`, RNG builtins, `eval()`, and condition helpers on interpreter `A` while interpreter `B` is installed in TLS, and asserts that results come from `A`.

### 2. `Sys.setenv()` and `setwd()` break interpreter isolation by mutating process-global state

Affected code:

- `src/interpreter/builtins/system.rs:576-639`

Why this matters:

- `std::env::set_var()` and `std::env::set_current_dir()` mutate process-global state, not interpreter-local state.
- Two `Session`s in the same process will interfere with each other.
- The `unsafe` comment on `Sys.setenv()` explicitly assumes single-threaded use, which conflicts with the stated embedding/reentrancy goal.

Suggested fix:

- Move environment variables and working directory into `Interpreter` state, then resolve system builtins against that state.
- If some actions must remain process-global, surface that through an explicit host API and document it as a deliberate divergence, not as silent shared mutation.

Suggested tests:

- Two `Session`s in one process.
- One changes env/cwd.
- The other must not observe the change unless an explicit shared-host mode is enabled.

### 3. `<<-` creates missing bindings in the base environment instead of the global environment

Affected code:

- `src/interpreter/environment.rs:61-75`

Why this matters:

- `set_super()` recursively walks parents until there is no parent left.
- In this tree the global environment’s parent is the base environment.
- That means a missing `<<-` target is created in base, not global.
- This can corrupt builtin lookup and make user code accidentally overwrite base bindings.

Confirmed reproduction:

```r
f <- function() { x <<- 1 }
f()
c(global_has = any(ls(globalenv()) == "x"), base_has = any(ls(baseenv()) == "x"))
```

Observed output:

```text
[1] FALSE TRUE
```

Suggested fix:

- Treat the global environment as the creation boundary for super-assignment.
- Search parents for an existing binding.
- If none is found before reaching global, create the binding in global, not in base.
- Add a regression test that also checks `<<-` does not overwrite a base builtin unless that builtin was the symbol explicitly found during the search.

### 4. Builtin `min_args` metadata is never enforced

Affected code:

- `src/interpreter/call_eval.rs:226-247`
- Example builtin declaration: `src/interpreter/builtins.rs:458-463`

Why this matters:

- The dispatcher enforces only `max_args`.
- Many builtins are annotated with `min_args`, but calls with too few arguments fall through into arbitrary builtin-specific behavior.
- That produces silently wrong answers instead of argument errors.

Confirmed reproduction:

```r
length()
```

Observed output:

```text
[1] 0
```

Expected behavior is an error for a missing required argument.

Suggested fix:

- Add a symmetric `ensure_builtin_min_arity()` and enforce it alongside the max-arity check.
- Add proc-macro metadata tests for both too-few and too-many arguments.
- Audit builtins that currently rely on `args.first().unwrap_or(...)` because some of those paths are masking missing-argument bugs.

### 5. Closure argument matching is far too weak for real R code

Affected code:

- `src/interpreter/arguments.rs:43-71`

Why this matters:

- Matching is exact-name-first, then positional.
- There is no partial matching.
- There is no unused-argument error when no `...` is present.
- There is no duplicate-match detection.
- That will break a large amount of CRAN code, especially code that relies on partial argument names or on R’s normal diagnostics for unused arguments.

Confirmed repros:

```r
f <- function(alpha, beta) c(alpha, beta)
f(al = 1, b = 2)
```

Observed result:

```text
Error: object 'alpha' not found
```

and

```r
f <- function(alpha, beta) c(alpha, beta)
f(gamma = 1, 2)
```

Observed result:

```text
Error: object 'beta' not found
```

That second case should be diagnosed as an unused argument, not as an unbound symbol inside the body.

Suggested fix:

- Implement R’s three-pass matching model:
- exact name
- partial name for unmatched formals
- positional for remaining unmatched formals
- After matching, error on leftovers when no `...` is present.
- Preserve missingness separately from default-value materialization.
- Add conformance tests for exact, partial, positional-after-named, duplicate, and unused-argument cases.

### 6. `system.time()` measures the already-evaluated result, not the expression

Affected code:

- `src/interpreter/builtins/interp.rs:655-667`

Why this matters:

- `system.time()` is implemented as an eager interpreter builtin.
- Call arguments are evaluated before the builtin sees them.
- The timer starts after the target expression has already run.

Confirmed reproduction:

```r
system.time({ x <- 0; for (i in 1:100000) x <- x + i; x })
```

Observed output:

```text
[1] 0.000000292 0 0.000000292
```

That is measuring only the trivial wrapper, not the loop.

Suggested fix:

- Convert `system.time()` into a pre-eval builtin.
- Time evaluation of the unevaluated expression inside the builtin.
- Add a regression test that proves the measured wall clock is meaningfully non-zero for a non-trivial expression.

### 7. Assignment, subsetting, and arithmetic routinely collapse types and strip attributes

Affected code:

- `src/interpreter/assignment.rs:114-221`
- `src/interpreter/indexing.rs:163-205`
- `src/interpreter/indexing.rs:385-507`
- `src/interpreter/ops.rs:203-316`
- `src/interpreter/builtins.rs:219-327`

Why this matters:

- Large parts of the evaluator convert through `to_doubles()` or `to_integers()` and then rebuild a fresh vector.
- That destroys storage mode, `names`, `dim`, `dimnames`, `class`, and any other attributes that should survive the operation.
- This is one of the biggest remaining correctness blockers for package compatibility.

Confirmed repros:

```r
x <- 1L
x[1] <- 2L
typeof(x)
```

Observed:

```text
[1] "double"
```

```r
m <- matrix(1:4, 2, 2)
m[1] <- 9L
is.null(dim(m))
```

Observed:

```text
[1] TRUE
```

```r
m <- matrix(1:4, 2, 2)
typeof(m[1, ])
```

Observed:

```text
[1] "double"
```

```r
m <- matrix(c("a", "b", "c", "d"), 2, 2)
m[1, 1]
```

Observed:

```text
[1] NA
```

```r
m <- matrix(1:4, 2, 2)
is.null(dim(m + 1))
```

Observed:

```text
[1] TRUE
```

```r
names(c(a = 1, b = 2))
```

Observed:

```text
NULL
```

Suggested fix:

- Introduce typed vector transformation helpers that preserve storage mode and copy/merge attributes intentionally.
- Centralize attribute propagation rules rather than rebuilding plain vectors ad hoc in each operator.
- Make matrix and array operations preserve `dim`/`dimnames`.
- Make `c()` preserve names from named arguments and existing element names.

Suggested tests:

- Replacement on integer, logical, character, complex, and raw vectors.
- Matrix/array subsetting and arithmetic with `dimnames`.
- Named concatenation through `c()`.

### 8. Subsetting semantics are still far from R in several important ways

Affected code:

- `src/interpreter/indexing.rs:63-75`
- `src/interpreter/indexing.rs:170-191`
- `src/interpreter/indexing.rs:333-382`
- `src/interpreter/indexing.rs:385-582`

Why this matters:

- `[` and `[[` are foundational.
- The current implementation gets several high-frequency cases wrong:
- logical index recycling
- mixed positive/negative index validation
- matrix dimname lookups
- data-frame row-name preservation
- some `[[` out-of-bounds semantics

Confirmed repros:

```r
x <- 1:4
x[c(TRUE, FALSE)]
```

Observed:

```text
[1] 1
```

Expected behavior is logical recycling, yielding `1 3`.

```r
x <- 1:3
x[c(-1, 2)]
```

Observed:

```text
[1] NA 2
```

Expected behavior is an error for mixing negative and positive subscripts.

```r
m <- matrix(1:4, 2, 2, dimnames = list(c("r1", "r2"), c("c1", "c2")))
m["r1", "c1"]
```

Observed:

```text
numeric(0)
```

Expected behavior is lookup by dimnames.

```r
df <- data.frame(a = 1:3, b = 4:6)
row.names(df[2:3, ])
```

Observed:

```text
[1] "1" "2"
```

Expected behavior is preservation of the selected row names.

Suggested fix:

- Add a proper index normalization layer shared by vectors, matrices, arrays, and data frames.
- Validate mixed-sign and zero-index cases before indexing.
- Recycle logical masks to the target length.
- Support character indexing through `names`/`dimnames`.
- Preserve row names when subsetting data frames.
- Add explicit tests for `[[` numeric out-of-bounds and named misses.

### 9. Top-level `NULL` results are always hidden

Affected code:

- `src/session.rs:64-69`

Why this matters:

- Visibility is currently `!is_invisible_result(expr) && !value.is_null()`.
- That hides ordinary visible `NULL` results in the REPL and `-e` mode.
- GNU R prints `NULL` unless the value was made invisible.

Confirmed reproduction:

```sh
target/debug/r -e 'NULL'
```

Observed behavior: no output.

Suggested fix:

- Remove the `!value.is_null()` suppression.
- Let visibility be driven by evaluation semantics, not by value kind.
- Add tests for `NULL`, `invisible(NULL)`, assignment, and loops.

### 10. The parse-corpus test does not fail on parse regressions

Affected code:

- `tests/parse_corpus.rs:18-40`

Why this matters:

- The test counts parse failures but never asserts that the count is zero or below a baseline.
- As written, the test succeeds as long as files do not panic and the test directory is non-empty.
- That makes it much weaker than the comment implies.

Suggested fix:

- Assert zero parse failures for `tests/`.
- If a non-zero baseline is temporarily necessary, encode it explicitly so regressions still fail.
- Consider a separate ignored/opt-in CRAN corpus test with stored expectations.

### 11. `cargo clippy --all-targets --all-features -- -D warnings` currently fails

Affected code:

- `tests/load_save.rs:14`
- `tests/builtin_args.rs:15`
- `tests/rds.rs:14`
- `tests/parse_corpus.rs:24`

Why this matters:

- The project instructions explicitly want zero clippy warnings before commits.
- The current tree already misses that bar.

Observed failures:

- `&PathBuf` should be `&Path` in multiple test helpers
- needless borrow in `tests/parse_corpus.rs`

Suggested fix:

- Fix the test helpers now.
- Put `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` into CI if they are not already enforced externally.

## Secondary fixes and suggestions

### 12. `Sys.which()` is not portable

Affected code:

- `src/interpreter/builtins/system.rs:591-614`

Problems:

- Splits `PATH` with `':'` directly instead of `std::env::split_paths()`
- Ignores Windows `PATHEXT`
- Returns empty strings rather than matching R’s platform behavior precisely

Suggested fix:

- Use `split_paths()`
- On Windows, search `PATHEXT`
- Add platform-gated tests

### 13. REPL startup can crash on history-file issues

Affected code:

- `src/main.rs:65-72`

Problem:

- `FileBackedHistory::with_file(...).expect(...)` aborts the whole REPL if the history file cannot be opened.

Suggested fix:

- Fall back to in-memory history and print a warning instead of panicking.

### 14. A few large files are still doing too much at once

Notable sizes:

- `src/interpreter/builtins.rs` — 3212 lines
- `src/interpreter/value.rs` — 1157 lines
- `src/parser.rs` — 906 lines
- `src/parser/diagnostics.rs` — 702 lines

Why this matters:

- The repo has already moved away from `mod.rs`, but these files are still broad enough that review and refactoring are getting expensive.
- The biggest semantic bugs above are harder to fix cleanly because logic is spread across very large mixed-responsibility files.

Suggested fix:

- Split builtin families further by domain and by semantic layer.
- Extract subsetting/index normalization into its own typed module.
- Extract attribute propagation into dedicated helpers.
- Extract parser builder helpers by grammar tier or expression family.

## Recommended order of work

1. Remove ambient-TLS dependence from public evaluation paths.
2. Fix process-global session leaks (`Sys.setenv`, `setwd`) or explicitly wall them off behind a host layer.
3. Correct `<<-` so missing symbols land in global, not base.
4. Enforce builtin `min_args`.
5. Rebuild closure argument matching to support exact, partial, positional, and unused-argument errors.
6. Introduce a single typed subsetting/replacement layer that preserves storage mode and attributes.
7. Make arithmetic and concatenation preserve attributes where R does.
8. Fix `system.time()` to evaluate inside the timed region.
9. Repair visibility of top-level `NULL`.
10. Strengthen tests: parse corpus assertions, semantic regressions, clippy gate.

## Summary

The project already has enough surface area that semantic drift now matters more than breadth. The biggest wins are not new builtin count; they are making evaluation state explicit, making subsetting/replacement type-stable and attribute-aware, and tightening function-call semantics. If those three areas are corrected, the existing builtin surface will become much more useful to real package code.
