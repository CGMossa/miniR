use std::process::Command;

#[test]
fn call_stack_builtins_work_for_nested_closures() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"f <- function(x = 1) {
  g <- function(y = 2, ...) {
    on.exit(cat("cleanup\n"))
    stopifnot(
      missing(y),
      missing(...),
      missing(..1),
      nargs() == 0L,
      sys.nframe() == 2L,
      identical(deparse(sys.call()), "g()"),
      identical(deparse(sys.call(1)), "f()"),
      identical(body(sys.function()), body(g)),
      identical(body(sys.function(1)), body(f)),
      identical(sys.parents(), 0:1),
      length(sys.calls()) == 2L,
      length(sys.frames()) == 2L,
      identical(environmentName(sys.frame()), "R_GlobalEnv"),
      identical(sort(ls(parent.frame())), c("g", "x")),
      grepl("cat(", deparse(sys.on.exit()), fixed = TRUE),
      grepl("cleanup", deparse(sys.on.exit()), fixed = TRUE)
    )
    NULL
  }
  g()
}
f()"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cleanup"), "unexpected stdout: {stdout}");
}

#[test]
fn missing_outside_a_function_reports_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", "missing(x)"])
        .output()
        .expect("failed to run miniR");

    assert!(!output.status.success(), "command unexpectedly succeeded");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("'missing(x)' did not find an argument"),
        "unexpected stderr: {stderr}"
    );
}
