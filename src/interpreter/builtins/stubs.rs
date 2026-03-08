//! Noop stub builtins — not-yet-implemented functions that return their first
//! argument (or NULL). Each is auto-registered via `noop_builtin!`.

use newr_macros::noop_builtin;

// Core language
noop_builtin!("on.exit");
noop_builtin!("UseMethod", 1);

// Data structures
noop_builtin!("array");
noop_builtin!("rbind");
noop_builtin!("cbind");

// Apply family
noop_builtin!("apply", 3);
noop_builtin!("mapply", 2);
noop_builtin!("tapply", 3);
noop_builtin!("by", 3);
// Linear algebra
noop_builtin!("diag", 1);
noop_builtin!("crossprod", 1);
noop_builtin!("tcrossprod", 1);
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

// Factors & tables
noop_builtin!("tabulate", 1);
noop_builtin!("table");
noop_builtin!("factor");
noop_builtin!("levels", 1);
noop_builtin!("nlevels", 1);

// Function tools
noop_builtin!("Recall");

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

// Package management
noop_builtin!("loadNamespace", 1);
noop_builtin!("requireNamespace", 1);
noop_builtin!("installed.packages");
noop_builtin!("install.packages");

// System
noop_builtin!("Sys.which", 1);
noop_builtin!("system", 1);
noop_builtin!("system2", 1);
noop_builtin!("Sys.setenv");
noop_builtin!("setwd", 1);

// Raw bytes
noop_builtin!("rawToChar", 1);
noop_builtin!("charToRaw", 1);
noop_builtin!("rawShift", 2);

// Bitwise
noop_builtin!("bitwNot", 1);
noop_builtin!("bitwAnd", 2);
noop_builtin!("bitwOr", 2);
noop_builtin!("bitwXor", 2);
noop_builtin!("bitwShiftL", 2);
noop_builtin!("bitwShiftR", 2);

// Regex
noop_builtin!("reg.finalizer", 2);
noop_builtin!("regmatches", 2);
noop_builtin!("regexpr", 2);
noop_builtin!("gregexpr", 2);
noop_builtin!("regexec", 2);
noop_builtin!("Sys.glob", 1);
noop_builtin!("glob2rx", 1);

// Directory/file ops
noop_builtin!("list.files");
noop_builtin!("dir");
noop_builtin!("file.info", 1);
noop_builtin!("file.size", 1);
noop_builtin!("file.copy", 2);
noop_builtin!("file.rename", 2);
noop_builtin!("file.remove", 1);
noop_builtin!("file.create", 1);
noop_builtin!("dir.create", 1);
noop_builtin!("dir.exists", 1);
noop_builtin!("tempfile");
noop_builtin!("tempdir");
noop_builtin!("unlink", 1);

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

// Metaprogramming
noop_builtin!("parse");
noop_builtin!("eval", 1);
noop_builtin!("evalq", 1);
noop_builtin!("quote", 1);
noop_builtin!("substitute", 1);
noop_builtin!("bquote", 1);
noop_builtin!("call", 1);
noop_builtin!("expression");
noop_builtin!("body", 1);
noop_builtin!("formals", 1);
noop_builtin!("arity", 1);
noop_builtin!("args", 1);

// Call stack
noop_builtin!("sys.frame");
noop_builtin!("parent.frame");
noop_builtin!("sys.parents");
noop_builtin!("sys.calls");
noop_builtin!("sys.frames");
noop_builtin!("sys.on.exit");

// Path
noop_builtin!("normalizePath", 1);
noop_builtin!("path.expand", 1);
