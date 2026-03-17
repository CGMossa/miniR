use r::Session;

// region: new.env

#[test]
fn new_env_creates_empty_environment() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        stopifnot(is.environment(e))
        stopifnot(length(ls(envir = e)) == 0)
    "#,
    )
    .unwrap();
}

#[test]
fn new_env_with_parent() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        parent_e <- new.env(parent = emptyenv())
        assign("x", 42, envir = parent_e)
        child_e <- new.env(parent = parent_e)
        stopifnot(get("x", envir = child_e) == 42)
    "#,
    )
    .unwrap();
}

#[test]
fn new_env_accepts_hash_and_size() {
    let mut r = Session::new();
    // These parameters should be accepted without error
    r.eval_source(
        r#"
        e1 <- new.env(hash = TRUE, parent = emptyenv(), size = 29L)
        stopifnot(is.environment(e1))
        e2 <- new.env(hash = FALSE, parent = emptyenv(), size = 100L)
        stopifnot(is.environment(e2))
        # Positional args: hash, parent, size
        e3 <- new.env(TRUE, emptyenv(), 50L)
        stopifnot(is.environment(e3))
    "#,
    )
    .unwrap();
}

#[test]
fn new_env_null_parent_gives_empty_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = NULL)
        stopifnot(is.environment(e))
        # Parent should be NULL (i.e., it's an empty-like env)
        stopifnot(identical(parent.env(e), NULL))
    "#,
    )
    .unwrap();
}

// endregion

// region: environment()

#[test]
fn environment_returns_closure_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        f <- function() NULL
        environment(f) # should not error
        stopifnot(is.environment(environment(f)))
    "#,
    )
    .unwrap();
}

#[test]
fn environment_no_args_returns_calling_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function() {
            environment()
        }
        env <- f()
        stopifnot(is.environment(env))
    "#,
    )
    .unwrap();
}

#[test]
fn environment_of_non_function_returns_null() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(is.null(environment(42)))
        stopifnot(is.null(environment("hello")))
        stopifnot(is.null(environment(TRUE)))
    "#,
    )
    .unwrap();
}

// endregion

// region: environmentName()

#[test]
fn environment_name_global() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(environmentName(globalenv()) == "R_GlobalEnv")
    "#,
    )
    .unwrap();
}

#[test]
fn environment_name_empty_for_anonymous() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        stopifnot(environmentName(e) == "")
    "#,
    )
    .unwrap();
}

#[test]
fn environment_name_base() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(environmentName(baseenv()) == "base")
    "#,
    )
    .unwrap();
}

#[test]
fn environment_name_empty_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(environmentName(emptyenv()) == "R_EmptyEnv")
    "#,
    )
    .unwrap();
}

// endregion

// region: parent.env()

#[test]
fn parent_env_of_global_is_base() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        p <- parent.env(globalenv())
        stopifnot(is.environment(p))
        stopifnot(environmentName(p) == "base")
    "#,
    )
    .unwrap();
}

#[test]
fn parent_env_of_child() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        parent <- new.env(parent = emptyenv())
        child <- new.env(parent = parent)
        stopifnot(identical(parent.env(child), parent))
    "#,
    )
    .unwrap();
}

#[test]
fn parent_env_of_empty_is_null() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        stopifnot(is.null(parent.env(emptyenv())))
    "#,
    )
    .unwrap();
}

#[test]
fn parent_env_errors_on_non_env() {
    let mut r = Session::new();
    let err = r
        .eval_source("parent.env(42)")
        .expect_err("should error on non-environment");
    assert!(
        err.to_string().contains("not an environment"),
        "unexpected error: {err}"
    );
}

// endregion

// region: parent.frame()

#[test]
fn parent_frame_returns_caller_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function() {
            x <- 99
            g()
        }
        g <- function() {
            pf <- parent.frame()
            # parent.frame() should give us f's environment, which has x=99
            get("x", envir = pf)
        }
        result <- f()
        stopifnot(result == 99)
    "#,
    )
    .unwrap();
}

#[test]
fn parent_frame_n_goes_back_multiple() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        outer <- function() {
            outer_var <- "hello"
            middle()
        }
        middle <- function() {
            inner()
        }
        inner <- function() {
            # parent.frame(2) should go back 2 levels to outer
            pf <- parent.frame(2)
            get("outer_var", envir = pf)
        }
        result <- outer()
        stopifnot(result == "hello")
    "#,
    )
    .unwrap();
}

// endregion

// region: sys.call() / sys.function()

#[test]
fn sys_call_returns_current_call() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x, y) {
            deparse(sys.call())
        }
        result <- f(1, 2)
        stopifnot(result == "f(1, 2)")
    "#,
    )
    .unwrap();
}

#[test]
fn sys_call_with_index() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function() {
            g()
        }
        g <- function() {
            deparse(sys.call(1))
        }
        result <- f()
        stopifnot(result == "f()")
    "#,
    )
    .unwrap();
}

#[test]
fn sys_function_returns_current_function() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x) {
            fn <- sys.function()
            is.function(fn)
        }
        stopifnot(f(1))
    "#,
    )
    .unwrap();
}

#[test]
fn sys_function_with_index() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function() {
            g()
        }
        g <- function() {
            fn <- sys.function(1)
            # sys.function(1) should be f
            identical(body(fn), body(f))
        }
        stopifnot(f())
    "#,
    )
    .unwrap();
}

// endregion

// region: match.arg()

#[test]
fn match_arg_exact_match() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match.arg("linear", c("linear", "quadratic", "cubic"))
        stopifnot(result == "linear")
    "#,
    )
    .unwrap();
}

#[test]
fn match_arg_partial_match() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match.arg("lin", c("linear", "quadratic", "cubic"))
        stopifnot(result == "linear")
    "#,
    )
    .unwrap();
}

#[test]
fn match_arg_null_returns_first() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match.arg(NULL, c("linear", "quadratic", "cubic"))
        stopifnot(result == "linear")
    "#,
    )
    .unwrap();
}

#[test]
fn match_arg_ambiguous_partial_errors() {
    let mut r = Session::new();
    let err = r
        .eval_source(r#"match.arg("l", c("linear", "logarithmic"))"#)
        .expect_err("ambiguous partial match should error");
    assert!(
        err.to_string().contains("should be one of"),
        "unexpected error: {err}"
    );
}

#[test]
fn match_arg_no_match_errors() {
    let mut r = Session::new();
    let err = r
        .eval_source(r#"match.arg("xyz", c("linear", "quadratic"))"#)
        .expect_err("no match should error");
    assert!(
        err.to_string().contains("should be one of"),
        "unexpected error: {err}"
    );
}

#[test]
fn match_arg_several_ok_true() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match.arg(c("linear", "cubic"), c("linear", "quadratic", "cubic"), several.ok = TRUE)
        stopifnot(length(result) == 2)
        stopifnot(result[1] == "linear")
        stopifnot(result[2] == "cubic")
    "#,
    )
    .unwrap();
}

#[test]
fn match_arg_several_ok_false_rejects_vector() {
    let mut r = Session::new();
    let err = r
        .eval_source(r#"match.arg(c("linear", "cubic"), c("linear", "quadratic", "cubic"))"#)
        .expect_err("several.ok=FALSE should reject length>1 arg");
    assert!(
        err.to_string().contains("should be one of"),
        "unexpected error: {err}"
    );
}

#[test]
fn match_arg_several_ok_with_partial() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- match.arg(c("lin", "cub"), c("linear", "quadratic", "cubic"), several.ok = TRUE)
        stopifnot(length(result) == 2)
        stopifnot(result[1] == "linear")
        stopifnot(result[2] == "cubic")
    "#,
    )
    .unwrap();
}

// endregion

// region: missing()

#[test]
fn missing_detects_unsupplied_arg() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x, y) {
            missing(y)
        }
        stopifnot(f(1))
        stopifnot(!f(1, 2))
    "#,
    )
    .unwrap();
}

#[test]
fn missing_with_default_still_missing() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x, y = 10) {
            missing(y)
        }
        stopifnot(f(1))
        stopifnot(!f(1, 20))
    "#,
    )
    .unwrap();
}

#[test]
fn missing_dots() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(...) {
            missing(...)
        }
        stopifnot(f())
        stopifnot(!f(1))
    "#,
    )
    .unwrap();
}

#[test]
fn missing_outside_function_errors() {
    let mut r = Session::new();
    let err = r
        .eval_source("missing(x)")
        .expect_err("missing outside function should error");
    assert!(
        err.to_string().contains("did not find an argument"),
        "unexpected error: {err}"
    );
}

// endregion

// region: nargs()

#[test]
fn nargs_counts_supplied_args() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x, y, z) nargs()
        stopifnot(f(1) == 1L)
        stopifnot(f(1, 2) == 2L)
        stopifnot(f(1, 2, 3) == 3L)
    "#,
    )
    .unwrap();
}

#[test]
fn nargs_zero_when_no_args() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x, y) nargs()
        stopifnot(f() == 0L)
    "#,
    )
    .unwrap();
}

#[test]
fn nargs_with_defaults_counts_only_supplied() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(x = 1, y = 2, z = 3) nargs()
        stopifnot(f() == 0L)
        stopifnot(f(10) == 1L)
        stopifnot(f(10, 20) == 2L)
    "#,
    )
    .unwrap();
}

// endregion

// region: Integration tests combining multiple builtins

#[test]
fn environment_chain_traversal() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e1 <- new.env(hash = TRUE, parent = emptyenv(), size = 10L)
        e2 <- new.env(parent = e1)
        e3 <- new.env(parent = e2)
        # Walk back the chain: e3 -> e2 -> e1 -> emptyenv() -> NULL
        stopifnot(identical(parent.env(e3), e2))
        stopifnot(identical(parent.env(parent.env(e3)), e1))
        stopifnot(environmentName(parent.env(parent.env(parent.env(e3)))) == "R_EmptyEnv")
        stopifnot(is.null(parent.env(parent.env(parent.env(parent.env(e3))))))
    "#,
    )
    .unwrap();
}

#[test]
fn combined_missing_nargs_match_arg() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        f <- function(method = c("linear", "quadratic"), verbose = FALSE) {
            n <- nargs()
            m <- missing(method)
            if (m) {
                method <- match.arg(NULL, c("linear", "quadratic"))
            } else {
                method <- match.arg(method, c("linear", "quadratic"))
            }
            list(n = n, missing_method = m, method = method)
        }
        r1 <- f()
        stopifnot(r1$n == 0)
        stopifnot(r1$missing_method)
        stopifnot(r1$method == "linear")
        r2 <- f("quad")
        stopifnot(r2$n == 1)
        stopifnot(!r2$missing_method)
        stopifnot(r2$method == "quadratic")
    "#,
    )
    .unwrap();
}

#[test]
fn sys_call_in_nested_functions() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        outer <- function(a) {
            inner(a + 1)
        }
        inner <- function(b) {
            list(
                inner_call = deparse(sys.call()),
                outer_call = deparse(sys.call(1)),
                nframe = sys.nframe()
            )
        }
        result <- outer(10)
        stopifnot(result$inner_call == "inner(a + 1)")
        stopifnot(result$outer_call == "outer(10)")
        stopifnot(result$nframe == 2L)
    "#,
    )
    .unwrap();
}

// endregion
