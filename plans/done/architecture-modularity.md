# Architecture Modularity Review

This review focuses on making miniR easier to extend without relying on hidden
knowledge about evaluator internals, builtin dispatch conventions, or thread-local
state.

## Summary

The current top-level split is directionally right:

- parser
- interpreter
- builtins
- values
- environments

The real maintenance pressure is not the directory layout. It is concentrated
in a few control-plane files:

- `src/main.rs` still owns the product boundary
- `src/interpreter.rs` still owns most evaluation behavior
- `src/interpreter/builtins.rs` still owns builtin registration plus many core helpers
- `minir-macros/src/lib.rs` only solves registration, not builtin ergonomics

That means extension work still depends on remembering a set of conventions:

- which builtin registry a function belongs in
- when to use `with_interpreter()`
- how named args are decoded by hand
- which behaviors live in the evaluator instead of the builtin layer

## Recommended Changes

1. Add a real library boundary.

   Create `src/lib.rs` and move the public parser/interpreter entrypoints there.
   Keep `src/main.rs` as a thin CLI wrapper around library APIs.

   This should expose a small embedding surface such as:

   - `Interpreter::new()`
   - `Interpreter::eval_str()`
   - `Interpreter::eval_file()`
   - `parse_program()`
   - output/visibility helpers for REPL and tests

   Why this matters:

   - the project goal explicitly includes reentrant embedding
   - integration tests currently shell out to the binary instead of driving the interpreter directly
   - CLI concerns and engine concerns are still coupled

2. Replace the three builtin registries with one descriptor model.

   Today builtin registration is split across:

   - eager builtins
   - interpreter builtins
   - pre-eval builtins

   Evaluation then does runtime linear scans over those registries and installs
   placeholder functions for non-eager builtins.

   Replace that with one descriptor type:

   ```rust
   struct BuiltinDescriptor {
       name: &'static str,
       aliases: &'static [&'static str],
       min_args: usize,
       max_args: Option<usize>,
       eval: EvalStrategy,
       handler: BuiltinHandler,
   }

   enum EvalStrategy {
       Eager,
       Interpreter,
       PreEval,
   }
   ```

   Build a name-to-descriptor map once during interpreter initialization.

   Benefits:

   - no placeholder builtin hack
   - no repeated registry scans on every call
   - one place to attach future metadata like docs, feature gates, categories, or visibility rules

3. Split `Interpreter` evaluation into focused submodules.

   `src/interpreter.rs` is now the dominant knowledge bottleneck. It contains:

   - expression evaluation
   - call dispatch
   - argument matching
   - S3 dispatch
   - control flow
   - indexing and replacement
   - vector operations
   - matrix operations

   Keep `Interpreter` state in `src/interpreter.rs`, but move behavior into
   submodules such as:

   - `interpreter/eval/literals.rs`
   - `interpreter/eval/control.rs`
   - `interpreter/eval/call.rs`
   - `interpreter/eval/assign.rs`
   - `interpreter/eval/index.rs`
   - `interpreter/eval/s3.rs`
   - `interpreter/eval/vector_ops.rs`
   - `interpreter/eval/matrix_ops.rs`

   The goal is not to hide methods. The goal is to make the ownership of each
   semantic area obvious.

4. Introduce a first-class builtin context instead of ad hoc TLS access.

   The current builtin split forces `interp.rs` and `pre_eval.rs` to reach back
   into the evaluator through `with_interpreter()`.

   Add a small context type:

   ```rust
   struct BuiltinContext<'a> {
       interp: &'a Interpreter,
       env: &'a Environment,
       call_expr: Option<&'a Expr>,
   }
   ```

   Then make interpreter-aware builtins receive that context directly.

   Keep TLS as the compatibility path for code that truly needs it, but stop
   making TLS the primary way builtin code talks to the interpreter.

   Benefits:

   - less hidden global coupling
   - easier unit testing
   - easier future move toward a non-TLS embedding API

5. Add a shared argument decoding layer for builtins.

   A large amount of builtin code manually reimplements:

   - named lookup
   - positional fallback
   - scalar coercion
   - boolean flag handling
   - environment extraction
   - repeated error strings for invalid arguments

   Add a helper layer, for example:

   - `CallArgs`
   - `ArgCursor`
   - `ArgDecoder`

   The API should support patterns like:

   - `args.string("file")?`
   - `args.flag("header").default(true)`
   - `args.integer("n")?`
   - `args.environment("envir").or_current()`

   This is one of the highest-leverage cleanup items because it removes boilerplate
   from almost every future builtin.

6. Extract builtin registration, constants, and shared helpers out of `builtins.rs`.

   `src/interpreter/builtins.rs` currently mixes:

   - registry declarations
   - registration logic
   - core helpers
   - constant installation
   - a large grab-bag of builtin implementations

   Split it into clearer ownership:

   - `builtins/registry.rs`
   - `builtins/constants.rs`
   - `builtins/args.rs`
   - `builtins/helpers.rs`
   - keep feature families in their existing modules

   The current submodules are useful, but the root builtin file is still doing too much.

7. Separate parser diagnostics from parsing/lowering.

   `src/parser.rs` currently mixes:

   - pest entrypoints
   - AST lowering
   - token classification
   - parse error formatting
   - fix suggestions

   Split it into:

   - `parser.rs` for public entrypoints
   - `parser/lower.rs` for pest-pair to AST conversion
   - `parser/diagnostics.rs` for `ParseError`, token classification, and suggestions

   This will make parser maintenance easier without changing the grammar.

8. Add a direct interpreter test harness once the library boundary exists.

   After `src/lib.rs` exists, add test helpers that can:

   - evaluate source strings directly
   - capture returned values
   - inspect environments
   - inspect errors without parsing stderr text

   Keep CLI smoke tests, but stop using them as the main integration-test surface.

## Proc-Macro Changes Worth Making

1. Replace the three builtin registration macros with one core macro.

   Keep the current public spellings if you want, but implement them as thin wrappers
   around one macro with explicit evaluation strategy:

   ```rust
   #[r_builtin(eval = "eager", name = "abs", min_args = 1)]
   #[r_builtin(eval = "interpreter", name = "eval", min_args = 1)]
   #[r_builtin(eval = "pre_eval", name = "quote", min_args = 1)]
   ```

   This keeps the registry model coherent and removes duplicated macro machinery.

2. Make builtin macros emit descriptor metadata, not only function pointers.

   The current macros only emit:

   - name
   - alias names
   - function pointer
   - `min_args`

   They should also be able to emit:

   - `max_args`
   - evaluation strategy
   - category
   - feature gate
   - stub/todo status

   That metadata is useful for:

   - better dispatch
   - help generation
   - docs
   - future package/base namespace introspection

3. Add a macro for typed builtin argument structs.

   The best new macro to add is not another registration macro. It is an argument
   decoding macro.

   Example shape:

   ```rust
   #[derive(FromCallArgs)]
   struct ReadCsvArgs {
       #[r_arg(pos = 0, name = "file")]
       file: String,
       #[r_arg(name = "header", default = true)]
       header: bool,
       #[r_arg(name = "sep", default = ",")]
       sep: String,
   }
   ```

   Or as an attribute on the builtin itself:

   ```rust
   #[builtin_args]
   struct SaveArgs { ... }
   ```

   This would remove a large amount of hand-written, inconsistent argument plumbing.

4. Add an explicit `todo_builtin!` or `stub_builtin!` macro.

   `noop_builtin!` is convenient, but returning the first argument or `NULL` is too
   implicit and too easy to forget about.

   Add a macro that registers stubs with:

   - a clear error message
   - optional tracking note
   - optional temporary placeholder behavior when that is truly intended

   That makes missing functionality visible instead of quietly permissive.

5. Validate builtin function signatures in the proc macros.

   The macros should reject handlers with the wrong argument shapes at compile time.
   That includes:

   - wrong number of parameters
   - wrong parameter types
   - wrong return type

   This is especially useful if the project moves to a context-based builtin API.

6. Generate a duplicate-name audit in test mode.

   It is easy to accidentally register:

   - the same builtin twice
   - the same alias twice
   - an alias that collides with a primary name

   The macro layer can emit metadata that feeds a generated test asserting that
   builtin names are unique.

## Proc-Macro Changes That Are Not Worth It Yet

- Do not generate semantic evaluator code from macros.
- Do not hide S3 dispatch rules behind macro attributes.
- Do not try to macro-generate vectorized arithmetic semantics.

Macros should remove registration and argument-decoding ceremony, not obscure the
runtime model.

## Suggested Implementation Order

1. Add `src/lib.rs` and move CLI logic behind public library entrypoints.
2. Introduce a unified builtin descriptor plus map-based lookup.
3. Add builtin context and stop relying on `with_interpreter()` inside builtin bodies.
4. Add typed argument decoding helpers or macros.
5. Split `interpreter.rs` and `parser.rs` by responsibility.
6. Add direct interpreter test helpers and gradually migrate integration coverage.
