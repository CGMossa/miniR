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
fn recall_computes_factorial_via_anonymous_recursion() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Recall() is the standard way to do recursion with anonymous functions
        fact <- function(n) {
            if (n <= 1) return(1)
            n * Recall(n - 1)
        }
        stopifnot(
            fact(1) == 1,
            fact(5) == 120,
            fact(10) == 3628800
        )
        "#,
    )
    .expect("Recall factorial failed");
}

#[test]
fn recall_passes_named_arguments() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Recall should forward named arguments
        my_sum <- function(acc = 0, n = 0) {
            if (n <= 0) return(acc)
            Recall(acc = acc + n, n = n - 1)
        }
        stopifnot(my_sum(n = 5) == 15)
        "#,
    )
    .expect("Recall named args failed");
}

#[test]
fn recall_outside_function_errors() {
    let mut s = Session::new();
    let err = s
        .eval_source("Recall(1)")
        .expect_err("Recall outside a function should fail");
    assert!(
        err.to_string().contains("outside a function"),
        "unexpected error: {err}"
    );
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
