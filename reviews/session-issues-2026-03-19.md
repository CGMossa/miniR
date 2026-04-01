# Issues Encountered — 2026-03-19 Session

## Bugs Found (not all fixed)

### 1. `<<-` with compound targets at global level

`msgs[[1]] <<- "hello"` at the top level doesn't work because `<<-`
delegates to `set_super()` which goes to the parent env (base), not
the current env. Inside functions it works correctly.

**Status**: FIXED (2026-04-01). `set_super()` now short-circuits when
called from global env, setting locally instead of recursing into base.

### 2. Chained replacement functions don't work

`body(f)[[2]][[2]] <- val` causes "invalid assignment target". This is
a multi-level replacement chain: `body<-`(f, `[[<-`(`[[`(body(f), 2), 2, val)).
Found in Matrix/coerce.R and Matrix/posdef.R.

**Status**: FIXED (2026-04-01). Implemented `Language::set_element()` for
`[[<-` on language objects, plus `rvalue_to_expr()` for proper round-tripping.

### 3. GNU R base packages can't be sourced

Base R packages (base, methods, stats, etc.) use `.Internal()` and
`.addCondHands` and other primitives that miniR can't implement. These
should always be skipped when testing CRAN sourcing.

**Status**: `.Internal` and `.Call` are now stubbed (error gracefully),
but base package R files define the R-level wrappers for these
primitives, so they fail at definition time.

### 4. `tryCatch` didn't catch recursion-limit errors

When eval hits the 256-depth limit, the error escaped `tryCatch`
because the handler itself couldn't execute (depth still at limit).

**Status**: FIXED — reset `eval_depth` to 0 when entering error handlers.

### 5. Shell escaping of R code in `-e` flag

zsh mangles `!` in R code passed via `-e`. `if (!x)` becomes
`if ('')x)` or similar. This caused many debugging headaches.

**Workaround**: Write R code to a temp file and `source()` it instead
of using `-e` with complex expressions.

### 6. No `timeout` command on macOS

`timeout` and `gtimeout` aren't available by default on macOS. This
caused a background process to hang indefinitely when testing CRAN
packages in subprocesses.

**Workaround**: Use Rust-level `catch_unwind` + per-Session isolation
(the cran_source_corpus.rs test) instead of shell-level timeouts.

### 7. `pnorm(0)` precision

The hand-rolled erfc approximation (Abramowitz & Stegun) gave
~1.5e-7 accuracy. `pnorm(0) == 0.5` was FALSE.

**Status**: FIXED — replaced with `libm::erfc` (machine precision).

### 8. `is.*` type predicates were aliased to `is.null`

`is.ordered`, `is.call`, `is.symbol`, `is.name`, `is.expression`,
`is.pairlist` all returned TRUE only for NULL.

**Status**: FIXED in earlier session.

### 9. Rd parser panics on UTF-8 multibyte chars

`peek_str(n)` sliced by byte count, hitting the middle of multibyte
chars like curly quotes (`'`).

**Status**: FIXED — `peek_str` now uses `char_indices().nth(n)`.

### 10. Duplicate match arms in rd.rs

The `\method`/`\S3method`/`\S4method` handlers kept getting duplicated
across merge conflicts, causing "unreachable pattern" errors.

**Status**: FIXED (multiple times — keeps recurring during merges).

## Process Issues

### Agents writing to main instead of worktrees

Some agents modified files in the main repo working tree instead of
their isolated worktree. This caused unexpected dirty state and merge
conflicts.

### Whole-file copies overwriting main changes

Copying entire files from worktrees (instead of applying patches)
overwrote unrelated changes made on main since the worktree branched.
This lost the `?base` namespace help feature once.

**Status**: Documented in CLAUDE.md — "never copy entire files".

### Disk space exhaustion from parallel agents

46GB of task output files accumulated in /private/tmp from agent runs.
Combined with 61GB target/ dir, this filled the disk.

### Background processes hanging without timeout

The shell-based CRAN testing approach hung because macOS doesn't have
`timeout`. The Rust integration test approach (catch_unwind + Session
isolation) is the correct solution.
