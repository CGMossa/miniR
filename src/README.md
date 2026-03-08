# src

The newr interpreter source code (Rust).

- `parser/r.pest` — PEG grammar for R (pest format)
- `parser/ast.rs` — AST node types
- `parser/mod.rs` — pest parse tree to AST conversion
- `interpreter/mod.rs` — tree-walking evaluator
- `interpreter/value.rs` — runtime value types (RValue, Vector, RError)
- `interpreter/environment.rs` — lexical scoping with Rc<RefCell<>>
- `interpreter/builtins.rs` — built-in function implementations
- `main.rs` — REPL and file execution entry point
