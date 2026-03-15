use r::Session;

#[test]
fn call_stack_builtins_work_for_nested_closures() {
    let mut s = Session::new();
    s.eval_source(
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
    )
    .expect("call stack tests failed");
}

#[test]
fn missing_outside_a_function_reports_an_error() {
    let mut s = Session::new();
    let err = s
        .eval_source("missing(x)")
        .expect_err("missing(x) outside a function should fail");

    assert!(
        err.to_string()
            .contains("'missing(x)' did not find an argument"),
        "unexpected error: {err}"
    );
}
