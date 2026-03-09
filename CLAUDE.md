# newr

An R interpreter written in Rust.

## Design Philosophy

We will diverge from R behavior when R behavior is absurd. This is not a drop-in replacement for GNU R — it is a new implementation that respects R's useful semantics while fixing the nonsensical ones. Breaking changes from GNU R will be documented. Don't worry about backwards compatibility with GNU R — correctness and clarity come first.

Error messages should be *better* than GNU R's — more informative, more specific, with suggestions for how to fix the problem. We have the advantage of building from scratch without legacy constraints. Every error message is an opportunity to teach the user something. Don't just say what went wrong — say why it went wrong and what to do about it.

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
- Formula (`~`) and complex numbers are parsed but stubbed in the interpreter
- Dependencies are vendored (`cargo vendor`) for LLM help and clarity

## Testing

- `./scripts/parse-test.sh <dir>` — test if all .R files in a directory parse without errors or panics
- `./scripts/parse-test.sh tests/` — run against our test corpus (should be 100%)
- `./scripts/parse-test.sh cran/` — run against top 200 CRAN packages + R base/recommended packages
- Use `--verbose` flag to see per-file results
- `just update-cran-test-packages` — clone/refresh the CRAN test packages in `cran/`

## Plans

- Don't use phases in plans — just list what needs to be done in a flat, prioritized order

## Commits

- Commit early and often — don't batch unrelated changes
- Each commit should be one logical change (one feature, one fix, one plan doc)
- Never mix justfile changes, builtins, plan docs, or type system changes in a single commit
- Write short imperative commit messages focused on the "why"
- Always end with `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`

## Module Structure

- NEVER use the `mod.rs` pattern (`foo/mod.rs`) — always use the `foo.rs` alongside `foo/` directory pattern instead
- Example: `builtins.rs` + `builtins/math.rs` + `builtins/strings.rs`
- If you find existing `mod.rs` files, refactor them to the `foo.rs` pattern

## Code Quality

- Before committing, always run: `cargo fmt`, `cargo clippy` (zero warnings), and `cargo test`
- `#[allow(dead_code)]` attributes are temporary scaffolding for stubbed features (formula, tilde, dotdot, etc.) — resolve them as features are implemented

## Reviews

- When things go wrong during development (test failures, runtime errors, unexpected behavior), write down what happened in `reviews/` as a markdown file
- These notes indicate missing features, edge cases, or bugs in the interpreter
- Each review file should describe: what was attempted, what went wrong, and what it implies about missing functionality
- Name files descriptively, e.g. `reviews/missing-named-arg-matching.md`

## Vendor Audit

- When dependencies change (new crates added, `just vendor` run), audit the vendor/ directory for R-relevant crates
- Write a `plans/` file for each vendored crate that could be integrated into the R interpreter
- Update `analysis/vendor-crate-audit.md` with the full categorization (integrated, high/medium/low priority, infrastructure)
- Use `just vendor` to re-vendor — never run `cargo vendor` directly (the justfile preserves README.md and .cargo-lock-hash)

## Tool Rules

- Do NOT tail or truncate `cargo vendor` output — let it run fully so the config snippet is visible
