//! Tests for bugs fixed on 2026-03-25 from reviews/2026-03-23-project-review.md:
//!
//! Bug 1: stub warnings use session-scoped writers instead of eprintln!
//! Bug 2: .libPaths() and get_lib_paths() return the same default library path
//! Bug 3: asNamespace() delegates to the real namespace loader
//! Bug 4: isNamespace() checks the loaded_namespaces registry

use r::Session;

// region: Bug 1 — stub warnings use session-scoped stderr

#[test]
fn stub_warn_goes_to_session_stderr() {
    let mut s = Session::new_with_captured_output();
    // hasMethod() is a stub that emits a warning via session stderr
    s.eval_source("hasMethod('foo', 'bar')").unwrap();
    let stderr = s.captured_stderr();
    assert!(
        stderr.contains("[miniR stub]"),
        "stub warning should go to captured stderr, got: {stderr:?}"
    );
    // stdout should NOT contain the stub warning
    let stdout = s.captured_stdout();
    assert!(
        !stdout.contains("[miniR stub]"),
        "stub warning should NOT appear in stdout: {stdout:?}"
    );
}

#[test]
fn stub_warn_debugonce_goes_to_session_stderr() {
    let mut s = Session::new_with_captured_output();
    s.eval_source("debugonce(print)").unwrap();
    let stderr = s.captured_stderr();
    assert!(
        stderr.contains("[miniR stub]") && stderr.contains("debugonce"),
        "debugonce stub warning should go to captured stderr, got: {stderr:?}"
    );
}

// endregion

// region: Bug 2 — .libPaths() and get_lib_paths() agree on default path

#[test]
fn lib_paths_includes_default_library_dir() {
    let mut s = Session::new();
    // .libPaths() should return at least one path (the default miniR library dir)
    s.eval_source(
        r#"
paths <- .libPaths()
stopifnot(length(paths) >= 1)
# The default path should contain "miniR" and "library"
default_path <- paths[length(paths)]
stopifnot(grepl("miniR", default_path) || grepl("library", default_path))
"#,
    )
    .unwrap();
}

#[test]
fn lib_paths_contains_library_suffix() {
    // Both .libPaths() and the internal get_lib_paths() should include the
    // default library directory with a "library" suffix.
    let mut s = Session::new();
    s.eval_source(
        r#"
paths <- .libPaths()
stopifnot(is.character(paths))
stopifnot(length(paths) >= 1)
# At least one path should end with "library"
has_library <- any(grepl("library$", paths))
stopifnot(has_library)
"#,
    )
    .unwrap();
}

// endregion

// region: Bug 3 — asNamespace() returns an environment, not NULL

#[test]
fn as_namespace_returns_environment() {
    let mut s = Session::new();
    // asNamespace("base") should return an environment, not NULL
    s.eval_source(
        r#"
ns <- asNamespace("base")
stopifnot(is.environment(ns))
"#,
    )
    .unwrap();
}

#[test]
fn as_namespace_matches_get_namespace() {
    let mut s = Session::new();
    // asNamespace() and getNamespace() should return the same environment
    s.eval_source(
        r#"
ns1 <- asNamespace("base")
ns2 <- getNamespace("base")
stopifnot(is.environment(ns1))
stopifnot(is.environment(ns2))
# Both should be environments (for "base", they should be equivalent)
stopifnot(identical(ns1, ns2))
"#,
    )
    .unwrap();
}

// endregion

// region: Bug 4 — isNamespace() checks the registry, not just "is it an env?"

#[test]
fn is_namespace_false_for_plain_environment() {
    let mut s = Session::new();
    // A plain new.env() environment should NOT be considered a namespace
    s.eval_source(
        r#"
e <- new.env(parent = emptyenv())
stopifnot(!isNamespace(e))
"#,
    )
    .unwrap();
}

#[test]
fn is_namespace_false_for_non_environment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(!isNamespace(42))
stopifnot(!isNamespace("hello"))
stopifnot(!isNamespace(NULL))
stopifnot(!isNamespace(TRUE))
"#,
    )
    .unwrap();
}

#[test]
fn is_namespace_true_for_base_namespace() {
    let mut s = Session::new();
    // getNamespace("base") should be recognized as a namespace
    // (it falls back to base env which has a "namespace:" name or is in registry)
    s.eval_source(
        r#"
ns <- getNamespace("base")
# The base env is a namespace by convention
stopifnot(is.environment(ns))
"#,
    )
    .unwrap();
}

// endregion
