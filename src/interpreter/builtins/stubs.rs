//! Stub builtins — not-yet-implemented functions that fail explicitly instead
//! of returning misleading placeholder values.

use minir_macros::stub_builtin;

// Core language (on.exit and missing — pre_eval.rs; UseMethod — builtins.rs)

// Data structures

// Apply family (apply, mapply, tapply, by — interp.rs)
// Linear algebra (norm, solve, outer — math.rs; crossprod, tcrossprod — math.rs)
stub_builtin!("qr", 1);
stub_builtin!("svd", 1);
stub_builtin!("eigen", 1);
// det and chol are implemented in math.rs

// Types (complex — math.rs; raw, rawShift, as.raw, is.raw — strings.rs)

// Factors & tables (factor, levels, nlevels, table, tabulate — builtins.rs)

// Function tools (Recall — stub with informative error, see builtins.rs)

// Error handling (withCallingHandlers — pre_eval.rs; condition constructors — builtins.rs)

// File I/O (scan — builtins.rs)
// Package management
stub_builtin!("loadNamespace", 1);
stub_builtin!("requireNamespace", 1);
stub_builtin!("installed.packages");
stub_builtin!("install.packages");

// Regex
stub_builtin!("reg.finalizer", 2);

// Connections
stub_builtin!("url", 1);
stub_builtin!("connection", 1);
stub_builtin!("close", 1);
stub_builtin!("open", 1);

// Metaprogramming (call, body, formals, args, Recall — builtins.rs; expression — pre_eval.rs)
stub_builtin!("arity", 1);
