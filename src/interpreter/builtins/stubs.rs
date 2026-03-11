//! Noop stub builtins — not-yet-implemented functions that return their first
//! argument (or NULL). Each is auto-registered via `noop_builtin!`.

use newr_macros::noop_builtin;

// Core language (on.exit — pre_eval.rs)
noop_builtin!("UseMethod", 1);

// Data structures

// Apply family (apply, mapply, tapply, by — interp.rs)
// Linear algebra (norm, solve, outer — math.rs; crossprod, tcrossprod — math.rs)
noop_builtin!("qr", 1);
noop_builtin!("svd", 1);
noop_builtin!("eigen", 1);
noop_builtin!("det", 1);
noop_builtin!("chol", 1);

// Types
noop_builtin!("complex");
noop_builtin!("raw");

// Factors & tables (factor, levels, nlevels, table, tabulate — builtins.rs)

// Function tools (Recall — stub with informative error, see builtins.rs)

// Error handling (withCallingHandlers — pre_eval.rs; condition constructors — builtins.rs)

// File I/O (scan — builtins.rs)
// Package management
noop_builtin!("loadNamespace", 1);
noop_builtin!("requireNamespace", 1);
noop_builtin!("installed.packages");
noop_builtin!("install.packages");

// Raw bytes
noop_builtin!("rawShift", 2);

// Regex
noop_builtin!("reg.finalizer", 2);

// Connections
noop_builtin!("url", 1);
noop_builtin!("connection", 1);
noop_builtin!("close", 1);
noop_builtin!("open", 1);

// Serialization
noop_builtin!("readRDS", 1);
noop_builtin!("saveRDS", 2);
noop_builtin!("load", 1);
noop_builtin!("save");

// Metaprogramming (call, body, formals, args, Recall — builtins.rs; expression — pre_eval.rs)
noop_builtin!("arity", 1);

// Call stack
noop_builtin!("sys.frame");
noop_builtin!("parent.frame");
noop_builtin!("sys.parents");
noop_builtin!("sys.calls");
noop_builtin!("sys.frames");
noop_builtin!("sys.on.exit");
