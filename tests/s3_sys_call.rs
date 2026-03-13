use std::process::Command;

#[test]
fn s3_methods_expose_method_calls_to_sys_call() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"f <- function(x) {
  cat("generic sys.call() =", deparse(sys.call()), "\n")
  cat("generic sys.call(0) =", deparse(sys.call(0)), "\n")
  UseMethod("f")
}
f.default <- function(x) {
  cat("method sys.call() =", deparse(sys.call()), "\n")
  cat("method sys.call(0) =", deparse(sys.call(0)), "\n")
  cat("method sys.call(1) =", deparse(sys.call(1)), "\n")
  cat("method sys.calls len =", length(sys.calls()), "\n")
  NULL
}
g <- function(x, ...) UseMethod("g")
g.foo <- function(x, ...) {
  cat("foo sys.call() =", deparse(sys.call()), "\n")
  cat("foo sys.call(1) =", deparse(sys.call(1)), "\n")
  NextMethod()
}
g.default <- function(x, ...) {
  cat("default sys.call() =", deparse(sys.call()), "\n")
  cat("default sys.call(1) =", deparse(sys.call(1)), "\n")
  cat("default sys.call(2) =", deparse(sys.call(2)), "\n")
  NULL
}
f(1)
g(structure(1, class = "foo"))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "generic sys.call() = f(1)",
        "generic sys.call(0) = f(1)",
        "method sys.call() = f.default(1)",
        "method sys.call(0) = f.default(1)",
        "method sys.call(1) = f(1)",
        "method sys.calls len = 2",
        "foo sys.call() = g.foo(structure(1, class = \"foo\"))",
        "foo sys.call(1) = g(structure(1, class = \"foo\"))",
        "default sys.call() = g.default(structure(1, class = \"foo\"))",
        "default sys.call(1) = g(structure(1, class = \"foo\"))",
        "default sys.call(2) = g.foo(structure(1, class = \"foo\"))",
    ] {
        assert!(
            stdout.contains(expected),
            "missing output `{expected}` in: {stdout}"
        );
    }
}

#[test]
fn usemethod_supports_computed_generic_names_and_unwinds() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"h <- function(x) {
  nm <- "h"
  cat("before =", deparse(sys.call()), "\n")
  UseMethod(nm, x)
  cat("after computed\n")
}
h.default <- function(x) {
  cat("method =", deparse(sys.call()), "\n")
  NULL
}
k <- function(x, y = 2, ...) {
  UseMethod("k")
  cat("after usemethod\n")
}
k.foo <- function(x, y = 10, ...) {
  stopifnot(missing(y), y == 10, nargs() == 1L)
  cat("missing method =", deparse(sys.call()), "\n")
  NULL
}
h(1)
k(structure(1, class = "foo"))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "before = h(1)",
        "method = h.default(1)",
        "missing method = k.foo(structure(1, class = \"foo\"))",
    ] {
        assert!(
            stdout.contains(expected),
            "missing output `{expected}` in: {stdout}"
        );
    }

    assert!(
        !stdout.contains("after computed"),
        "UseMethod() should not return to the generic body: {stdout}"
    );
    assert!(
        !stdout.contains("after usemethod"),
        "UseMethod() should unwind the generic after dispatch: {stdout}"
    );
}
