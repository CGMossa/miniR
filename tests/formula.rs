use std::process::Command;

#[test]
fn tilde_builds_formula_objects() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"rhs <- ~x + y
lhs <- z ~ x + y

check_formula <- function(f, expected) {
  stopifnot(
    typeof(f) == "language",
    mode(f) == "call",
    identical(class(f), "formula"),
    inherits(f, "formula"),
    is.environment(attr(f, ".Environment")),
    identical(sort(names(attributes(f))), c(".Environment", "class")),
    identical(deparse(unclass(f)), expected)
  )
}

check_formula(rhs, "~x + y")
check_formula(lhs, "z ~ x + y")"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
