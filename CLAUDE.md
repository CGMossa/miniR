# miniR

An R interpreter written in Rust.

## Goals

1. **Run top CRAN packages** — the interpreter should handle real-world R code from popular CRAN packages, not just toy examples
2. **Well-written code** — clean, idiomatic Rust; no hacks, no over-engineering
3. **Reentrant interpreter** — multiple R interpreters must coexist in the same process. No process-global statics. Thread-local storage (TLS) is the baseline for per-interpreter state. This enables embedding miniR as a library, running parallel R sessions, and testing interpreters in isolation.

## Design Philosophy

We will diverge from R behavior when R behavior is absurd. This is not a drop-in replacement for GNU R — it is a new implementation that respects R's useful semantics while fixing the nonsensical ones. Breaking changes from GNU R will be documented. Don't worry about backwards compatibility with GNU R — correctness and clarity come first.

Error messages should be *better* than GNU R's — more informative, more specific, with suggestions for how to fix the problem. We have the advantage of building from scratch without legacy constraints. Every error message is an opportunity to teach the user something. Don't just say what went wrong — say why it went wrong and what to do about it.

## Concurrency Rules

- **No process-global mutable statics** — put per-interpreter state on the `Interpreter` struct
- **`thread_local!` exists as infrastructure** — but builtins should use `BuiltinContext` to access the interpreter, not raw TLS via `with_interpreter()`
- **`Rc<RefCell<>>` is fine** — the interpreter is single-threaded per instance; no need for `Arc<Mutex<>>` unless explicitly sharing across threads
- When adding new state (RNG, temp dirs, env vars, working directory, options, etc.), put it on the `Interpreter` struct, not in a static
- Env vars and working directory are per-interpreter (not process-global) — use `interp.get_env_var()` / `interp.set_env_var()` / `interp.get_working_dir()` / `interp.set_working_dir()`

## Project Structure

- `src/lib.rs` — library boundary exposing interpreter, parser, repl, session
- `src/session.rs` — `Session` struct wrapping `Interpreter` for public API
- `src/main.rs` — thin CLI wrapper using Session API
- `src/repl.rs` — REPL support (highlighting, completion, validation, prompt)
- `src/parser/r.pest` — PEG grammar (pest), follows R Language Definition operator precedence
- `src/parser/ast.rs` — AST types
- `src/parser.rs` — pest pairs to AST conversion
- `src/parser/diagnostics.rs` — parse error formatting and fix suggestions
- `src/interpreter.rs` — tree-walking evaluator (core `eval_in` dispatch, ~330 lines)
- `src/interpreter/ops.rs` — arithmetic, comparison, logical, range, %in%, matmul
- `src/interpreter/assignment.rs` — `eval_assign` + all replacement semantics
- `src/interpreter/indexing.rs` — read-side vector/list/matrix/data-frame indexing
- `src/interpreter/control_flow.rs` — if/while/repeat/for/pipe evaluation
- `src/interpreter/call.rs` — `BuiltinContext`, `CallFrame`, `S3DispatchContext`
- `src/interpreter/call_eval.rs` — call evaluation and function dispatch
- `src/interpreter/arguments.rs` — three-pass closure argument binding (exact/partial/positional)
- `src/interpreter/s3.rs` — S3 method dispatch (UseMethod/NextMethod)
- `src/interpreter/value.rs` — `RValue`, `Vector`, `RError` types
- `src/interpreter/environment.rs` — lexical scoping with `Rc<RefCell<>>`
- `src/interpreter/builtins.rs` — 200+ built-in functions (unified `BuiltinDescriptor` dispatch)
- `src/interpreter/builtins/args.rs` — `CallArgs` helper for argument decoding
- `src/interpreter/builtins/datetime.rs` — date/time builtins (jiff crate)
- `tests/` — Rust integration tests + R test scripts
- `plans/` — dependency and design plans
- `reviews/` — development notes on bugs and missing features

## Key Decisions

- Base env (builtins) is parent of global env, matching R's env chain
- `T` and `F` are identifiers bound to TRUE/FALSE (reassignable), not literals
- `TRUE` and `FALSE` are keywords (not reassignable)
- `**` is a synonym for `^` (power)
- Function lookup in call position skips non-function bindings (R's findFun behavior)
- Formula (`~`) is parsed but stubbed in the interpreter
- Complex numbers are fully supported via `num-complex` (Vector::Complex, arithmetic, Re/Im/Mod/Arg/Conj)
- Dependencies are vendored (`cargo vendor`) for LLM help and clarity
- Make as many dependencies optional as possible, and let the default feature include all additive features
- `<<-` creates missing bindings in global env (not base)
- `print()` and `format()` are S3 generics — they dispatch to `print.Date`, `format.POSIXct`, etc.

## Known Parser Divergences from GNU R

- **Newline continuation in postfix chains**: `x\n(y)` parses as a call `x(y)`, not two expressions. Same for `x\n$y` and `pkg\n::foo`. Required for CRAN compat (7014/7014 files pass). See `reviews/parser-newline-continuation.md`.
- **`if...else` across newlines**: `if (TRUE) 1\nelse 2` is accepted (GNU R rejects when else is on a new line without braces).
- **`?` not embeddable**: `x <- ?sin` doesn't work — `?` is only at the top level of the precedence chain. Interactive-only, low priority.
- **Binary `?` drops RHS**: `foo ? bar` parses but the AST discards the topic. Help system not fully implemented.
- **`~~` and `:=` are parsed but stubbed**: `~~` (plotmath) evaluates to NULL; `:=` (walrus) has no runtime semantics yet.

## Testing

- `cargo test` — primary test command, runs all Rust integration tests
- `cargo clippy --all-targets --all-features -- -D warnings` — must pass with zero warnings
- **Every new feature should have tests planned** — either `stopifnot` assertions via `Session::eval_source` in a Rust integration test, or direct value checks via the Session API. Tests don't have to land in the same commit, but they should be planned and tracked. If an agent produces code without tests, note what needs coverage.
- `tests/smoke.rs` — end-to-end coverage of ops, assignment, indexing, datetime
- `tests/reentrancy.rs` — session isolation, nested eval, parallel threads
- `tests/parse_corpus.rs` — runs all .R files through parser, asserts no regressions. By default only scans `tests/` directory (fast). Set `MINIR_PARSE_CRAN=1` to include the full CRAN corpus from `cran/` (~7000 files, ~50s). The `cran/` directory must be present (use `just update-cran-test-packages`).
- `tests/argument_matching.rs` — three-pass matching conformance
- `./scripts/parse-test.sh <dir>` — test if all .R files in a directory parse without errors or panics
- `just update-cran-test-packages` — clone/refresh the CRAN test packages in `cran/`

## CI

- GitHub Actions: `.github/workflows/ci.yml`
- Runs on push to main and on PRs
- Steps: vendor dependencies, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`

## Plans

- Don't use phases in plans — just list what needs to be done in a flat, prioritized order

## Work Style

- Don't ask about priorities — just pick something from TODO.md and start working
- Everything on the TODO is a priority; forward progress on any item is good
- Bias toward action over discussion

## Commits

- Commit early and often — don't batch unrelated changes
- Each commit should be one logical change (one feature, one fix, one plan doc)
- Never mix `justfile` changes, builtins, plan docs, or type system changes in a single commit
- Write short imperative commit messages focused on the "why"
- Always end with `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`

## Module Structure

- NEVER use the `mod.rs` pattern (`foo/mod.rs`) — always use the `foo.rs` alongside `foo/` directory pattern instead
- Example: `builtins.rs` + `builtins/math.rs` + `builtins/strings.rs`
- If you find existing `mod.rs` files, refactor them to the `foo.rs` pattern

## Code Organization

- Use `// region: Description` and `// endregion` comments to delimit logical sections within a file
- If the section is large enough to warrant its own submodule, prefer that over region/endregion — add a module-level doc comment describing the module's purpose
- Do NOT use `// ── Section ──────` style section dividers

## Code Quality

- Before committing, always run in this order: `cargo fmt`, then `cargo clippy --all-targets --all-features -- -D warnings` (zero warnings), then `cargo test` — fmt must run first so clippy reports correct line numbers
- **No "pre-existing" warnings** — if you encounter a warning or error, fix it. There is no such thing as a pre-existing issue that can be ignored. Every warning is a bug to be fixed, not a known issue to be documented.
- `#[allow(dead_code)]` attributes are temporary scaffolding for stubbed features (formula, tilde, dotdot, etc.) — resolve them as features are implemented
- **No `#[non_exhaustive]`** — don't use the `non_exhaustive` attribute; it weakens exhaustive match checking and makes the codebase less robust
- **Prefer `From`/`TryFrom` over `as` casts** — use `TryFrom` and `From` trait conversions instead of `as`-casts; propagate the error rather than silently truncating or wrapping
- **Collect all errors, not just the first** — in operations that can fail at multiple points (e.g. vectorized ops, argument validation), collect all errors and propagate them together rather than bailing on the first one

## Reviews

- When things go wrong during development (test failures, runtime errors, unexpected behavior), write down what happened in `reviews/` as a markdown file
- These notes indicate missing features, edge cases, or bugs in the interpreter
- Each review file should describe: what was attempted, what went wrong, and what it implies about missing functionality
- Name files descriptively, e.g. `reviews/missing-named-arg-matching.md`

## Agent Worktrees

- Agents should always run in **worktrees** (`isolation: "worktree"`) so they don't collide with each other or main
- Agents should **remove `.cargo/config.toml`** in their worktree (`rm -f .cargo/config.toml`) so `cargo` fetches from crates.io instead of the vendor dir — this avoids "package not found" errors when agents add new dependencies
- After an agent finishes, the parent merges its work into main, then re-vendors with `just vendor-force`
- Never delete a worktree until its changes have been verified as merged into main

## Vendor Audit

- When dependencies change (new crates added, `just vendor` run), audit the vendor/ directory for R-relevant crates
- Write a `plans/` file for each vendored crate that could be integrated into the R interpreter
- Update `analysis/vendor-crate-audit.md` with the full categorization (integrated, high/medium/low priority, infrastructure)
- After adding a new dependency, run `cargo tree -p <dep>` to discover its transitive dependencies that might be useful for the interpreter, and write plans for any relevant ones
- Use `just vendor` to re-vendor — never run `cargo vendor` directly (the `justfile` preserves README.md and writes .cargo/config.toml)
- The vendor directory uses an absolute path in `.cargo/config.toml` — this is required because subagents and worktrees run from different working directories
- **Adding new dependencies**: `.cargo/config.toml` replaces crates-io with the vendor dir, so `cargo add` and `cargo update` fail for new crates. Workaround: (1) add the dep to `Cargo.toml` manually, (2) temporarily `mv .cargo/config.toml .cargo/config.toml.bak`, (3) run `cargo update`, (4) `mv .cargo/config.toml.bak .cargo/config.toml`, (5) `just vendor-force` to vendor the new crate
- **Cargo.lock merge conflicts**: When `Cargo.lock` has merge conflicts, don't try to resolve them manually — delete it and run `cargo generate-lockfile` to regenerate from scratch, then re-vendor

## File Deletion Safety

- **Never use `rm` or any permanent deletion command.**
- Always use a safe delete mechanism that moves files to the system trash/recycle bin instead of permanently removing them.
- This ensures files can be recovered if an action was incorrect, unintended, or unsafe.
- Use a trash command (e.g. `trash`, `gio trash`, `gvfs-trash`, or platform equivalent).
- If no trash utility is available, **stop and ask for guidance** instead of deleting.
- Permanent deletion is irreversible and unsafe in automated or agent-driven workflows. Using the trash provides a recovery path.

## Tool Rules

- Do NOT tail or truncate `cargo vendor` output — let it run fully so the config snippet is visible
- Never pipe cargo command output through `head` or `tail` — store the full log in a temp file, then read the relevant portion. If more issues surface later, you can go back to the logfile instead of re-running the command
