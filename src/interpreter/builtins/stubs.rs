//! Noop stub builtins — not-yet-implemented functions that return their first
//! argument (or NULL). Each is auto-registered via `noop_builtin!`.

use newr_macros::noop_builtin;

// Core language
noop_builtin!("on.exit");
noop_builtin!("UseMethod", 1);

// Data structures

// Apply family
noop_builtin!("apply", 3);
noop_builtin!("mapply", 2);
noop_builtin!("tapply", 3);
noop_builtin!("by", 3);
// Linear algebra
noop_builtin!("solve", 1);
noop_builtin!("qr", 1);
noop_builtin!("svd", 1);
noop_builtin!("eigen", 1);
noop_builtin!("det", 1);
noop_builtin!("chol", 1);
noop_builtin!("norm", 1);

// Types
noop_builtin!("complex");
noop_builtin!("raw");

// Factors & tables (factor, levels, nlevels, table, tabulate — builtins.rs)

// Function tools (Recall — stub with informative error, see builtins.rs)

// Error handling
noop_builtin!("withCallingHandlers", 1);
noop_builtin!("conditionMessage", 1);
noop_builtin!("conditionCall", 1);
noop_builtin!("simpleCondition", 1);
noop_builtin!("simpleError", 1);
noop_builtin!("simpleWarning", 1);
noop_builtin!("simpleMessage", 1);

// File I/O
noop_builtin!("scan");
noop_builtin!("file.info", 1);

// Package management
noop_builtin!("loadNamespace", 1);
noop_builtin!("requireNamespace", 1);
noop_builtin!("installed.packages");
noop_builtin!("install.packages");

// Raw bytes
noop_builtin!("rawShift", 2);

// Regex
noop_builtin!("reg.finalizer", 2);
noop_builtin!("Sys.glob", 1);

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
