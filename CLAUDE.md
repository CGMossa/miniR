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

- **No process-global mutable statics** — use `thread_local!` for interpreter state that builtins need to access (e.g. `with_interpreter()` pattern)
- **`thread_local!` is the baseline** — each thread gets its own interpreter instance, no cross-thread sharing needed for interpreter state
- **`Rc<RefCell<>>` is fine** — the interpreter is single-threaded per instance; no need for `Arc<Mutex<>>` unless explicitly sharing across threads
- When adding new state (RNG, temp dirs, options, etc.), put it on the `Interpreter` struct, not in a static

## Project Structure

- `src/parser/r.pest` — PEG grammar (pest), follows R Language Definition operator precedence
- `src/parser/ast.rs` — AST types
- `src/parser/mod.rs` — pest pairs to AST conversion
- `src/interpreter/mod.rs` — tree-walking evaluator
- `src/interpreter/value.rs` — RValue, Vector, RError types
- `src/interpreter/environment.rs` — lexical scoping with Rc<RefCell<>>
- `src/interpreter/builtins.rs` — built-in functions
- `src/main.rs` — REPL (reedline) + file execution CLI
- `tests/` — R test scripts
- `plans/` — dependency and design plans

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

## Testing

- `./scripts/parse-test.sh <dir>` — test if all .R files in a directory parse without errors or panics
- `./scripts/parse-test.sh tests/` — run against our test corpus (should be 100%)
- `./scripts/parse-test.sh cran/` — run against top 200 CRAN packages + R base/recommended packages
- Use `--verbose` flag to see per-file results
- `just update-cran-test-packages` — clone/refresh the CRAN test packages in `cran/`

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

- Before committing, always run: `cargo fmt`, `cargo clippy` (zero warnings), and `cargo test`
- `#[allow(dead_code)]` attributes are temporary scaffolding for stubbed features (formula, tilde, dotdot, etc.) — resolve them as features are implemented
- **No `#[non_exhaustive]`** — don't use the `non_exhaustive` attribute; it weakens exhaustive match checking and makes the codebase less robust
- **Prefer `From`/`TryFrom` over `as` casts** — use `TryFrom` and `From` trait conversions instead of `as`-casts; propagate the error rather than silently truncating or wrapping
- **Collect all errors, not just the first** — in operations that can fail at multiple points (e.g. vectorized ops, argument validation), collect all errors and propagate them together rather than bailing on the first one

## Reviews

- When things go wrong during development (test failures, runtime errors, unexpected behavior), write down what happened in `reviews/` as a markdown file
- These notes indicate missing features, edge cases, or bugs in the interpreter
- Each review file should describe: what was attempted, what went wrong, and what it implies about missing functionality
- Name files descriptively, e.g. `reviews/missing-named-arg-matching.md`

## Vendor Audit

- When dependencies change (new crates added, `just vendor` run), audit the vendor/ directory for R-relevant crates
- Write a `plans/` file for each vendored crate that could be integrated into the R interpreter
- Update `analysis/vendor-crate-audit.md` with the full categorization (integrated, high/medium/low priority, infrastructure)
- After adding a new dependency, run `cargo tree -p <dep>` to discover its transitive dependencies that might be useful for the interpreter, and write plans for any relevant ones
- Use `just vendor` to re-vendor — never run `cargo vendor` directly (the `justfile` preserves README.md and writes .cargo/config.toml)
- The vendor directory uses an absolute path in `.cargo/config.toml` — this is required because subagents and worktrees run from different working directories

## Tool Rules

- Do NOT tail or truncate `cargo vendor` output — let it run fully so the config snippet is visible
- Never pipe cargo command output through `head` or `tail` — store the full log in a temp file, then read the relevant portion. If more issues surface later, you can go back to the logfile instead of re-running the command
