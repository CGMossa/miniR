# Architecture Remediation Plan

This plan turns the architecture review into concrete follow-up work. It keeps
the priorities grouped by severity, but the execution order is flat and
incremental.

## P1

### P1. Binary-first product boundary

- Add `src/lib.rs` and make parser/interpreter/session APIs public from the
  library crate.
- Keep `src/main.rs` thin: CLI parsing, REPL setup, process exit codes, and
  printing only.
- Add a session-oriented API that owns an interpreter instance and can evaluate
  strings/files without shelling out to the binary.
- Move result-visibility logic out of `main.rs` into the library boundary.
- Add at least one direct library integration test that uses the session API
  instead of `Command::new(env!("CARGO_BIN_EXE_r"))`.

### P1. Hidden builtin-dispatch conventions

- Replace the three registry tuple types with a single builtin descriptor type.
- Include evaluation strategy as data instead of encoding it in which registry a
  handler lands in.
- Build a name-to-descriptor map once during interpreter initialization.
- Remove placeholder builtin installation once the descriptor-based dispatch is
  in place.
- Add a duplicate-name/alias audit test for builtin descriptors.

### P1. TLS as the practical builtin control plane

- Add an explicit builtin/evaluation context type for interpreter-aware code.
- Stop making `with_interpreter()` the primary way builtin handlers access the
  evaluator.
- Keep TLS as a compatibility layer for legacy handlers until they are migrated.
- Make the session API install its interpreter instance into TLS only as an
  implementation detail during evaluation, not as the public API surface.

## P2

### P2. `src/interpreter.rs` is still the evaluator choke point

- Split evaluator behavior by responsibility while keeping `Interpreter` state
  centralized.
- Start with the highest-churn areas:
  - call dispatch
  - argument matching
  - S3 dispatch
  - indexing/replacement
- Move vector/matrix ops behind dedicated evaluator submodules so builtin code
  and evaluator code stop bleeding together.

### P2. Builtin argument decoding is duplicated everywhere

- Add a shared builtin argument-decoding layer under `src/interpreter/builtins/`.
- Support common patterns directly:
  - named lookup
  - positional fallback
  - scalar coercion
  - defaults
  - environment extraction
  - flag decoding
- Migrate high-churn builtins first:
  - `interp.rs`
  - `io.rs`
  - `math.rs`
- Standardize error wording for invalid arguments through this layer.

### P2. Proc macros only solve registration

- Validate builtin handler signatures in the proc macros.
- Unify the three builtin attribute macros behind one shared internal model of
  evaluation strategy.
- Extend emitted metadata so future registry work can include:
  - max arity
  - evaluation strategy
  - aliases
  - category
  - stub status
- Add a better stub macro that reports intentional unimplemented behavior
  explicitly instead of silently returning the first argument.

## P3

### P3. Parser lowering and diagnostics are still coupled

- Split parser diagnostics out of `src/parser.rs`.
- Keep `parse_program()` as the stable entrypoint.
- Move `ParseError`, token classification, context building, and suggestion
  helpers into `src/parser/diagnostics.rs`.
- After diagnostics are separated, split AST lowering into a dedicated parser
  submodule if `src/parser.rs` still remains too broad.

## First Implementation Tranche

The first code tranche on top of this plan should deliver one concrete step from
each priority bucket:

- P1: add `src/lib.rs` plus a session API and convert `main.rs` to use it
- P2: add proc-macro signature validation so builtin conventions stop being
  implicit
- P3: split parser diagnostics into `src/parser/diagnostics.rs`

That tranche is intentionally chosen because it improves extension points
without forcing the much larger builtin-descriptor or evaluator-submodule
refactors into the same branch.
