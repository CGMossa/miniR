# 2026-03-23 Plans Review

Scope: review of plan quality and drift against the current codebase. I compared the plan docs to the current source tree rather than treating the docs as authoritative.

## Findings

### 1. `plans/README.md` still directs readers to at least one plan that is no longer an active plan

Severity: medium

- `plans/README.md` calls `call-stack.md` a "High-signal" plan to start with: [plans/README.md](/Users/elea/Documents/GitHub/newr/plans/README.md#L13).
- But `plans/call-stack.md` still says there is "no general call stack" and lists `missing()`, `parent.frame()`, `sys.call()`, and `on.exit()` as missing: [plans/call-stack.md](/Users/elea/Documents/GitHub/newr/plans/call-stack.md#L5).
- The codebase already has `call_stack` storage, helpers, and implementations/tests for those facilities: [src/interpreter/call.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/call.rs#L84), [src/interpreter/call_eval.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/call_eval.rs#L115), [src/interpreter/builtins/interp.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/interp.rs#L2106), [src/interpreter/builtins/pre_eval.rs](/Users/elea/Documents/GitHub/newr/src/interpreter/builtins/pre_eval.rs#L1029), [tests/env_scope_builtins.rs](/Users/elea/Documents/GitHub/newr/tests/env_scope_builtins.rs#L263).

Why this matters: the README correctly warns that plans drift, but still labels a historical document as a primary starting point. That increases the chance of duplicate work.

### 2. `plans/architecture-remediation.md` reads like an active roadmap, but its "first tranche" is mostly already shipped

Severity: medium

- The plan still says the first tranche should add `src/lib.rs`, a session API, proc-macro signature validation, and parser diagnostics extraction: [plans/architecture-remediation.md](/Users/elea/Documents/GitHub/newr/plans/architecture-remediation.md#L95).
- Those pieces already exist in-tree: [src/lib.rs](/Users/elea/Documents/GitHub/newr/src/lib.rs#L1), [src/session.rs](/Users/elea/Documents/GitHub/newr/src/session.rs#L79), [minir-macros/src/lib.rs](/Users/elea/Documents/GitHub/newr/minir-macros/src/lib.rs#L143), [src/parser/diagnostics.rs](/Users/elea/Documents/GitHub/newr/src/parser/diagnostics.rs).

Why this matters: the document is still phrased as a remediation queue, not a historical snapshot. Someone picking it up cold would spend time re-verifying work that has already landed.

### 3. `plans/testing.md` is not executable as written because it names the wrong Cargo binary

Severity: medium

- The `assert_cmd` example uses `Command::cargo_bin("miniR")`: [plans/testing.md](/Users/elea/Documents/GitHub/newr/plans/testing.md#L14).
- The actual package/binary name in the current repo is `r`: [Cargo.toml](/Users/elea/Documents/GitHub/newr/Cargo.toml#L11), [README.md](/Users/elea/Documents/GitHub/newr/README.md#L36).
- A repo-wide search shows no current test using `cargo_bin("miniR")`; the only hit is the plan itself: [plans/testing.md](/Users/elea/Documents/GitHub/newr/plans/testing.md#L14).

Why this matters: following the plan verbatim would generate a failing harness before it exercises any CLI behavior.

## Recommendation

- Mark stale plans explicitly as historical, or move them under a separate `historical/` subdirectory.
- Keep `plans/README.md` pointing only at plans whose "current state" sections still match the tree.
- Treat plan docs as maintainable artifacts: when a tranche lands, either update the plan or archive it immediately.

## Assumptions

- This was a documentation review only.
- I did not attempt to validate the plan snippets by running them.
