use r::Session;

// region: rm() / remove()

#[test]
fn rm_removes_single_variable() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- 42
        stopifnot(exists("x"))
        rm("x")
        stopifnot(!exists("x"))
    "#,
    )
    .unwrap();
}

#[test]
fn rm_removes_multiple_variables() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        a <- 1
        b <- 2
        c <- 3
        rm("a", "b")
        stopifnot(!exists("a"))
        stopifnot(!exists("b"))
        stopifnot(exists("c"))
    "#,
    )
    .unwrap();
}

#[test]
fn rm_with_list_argument() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- 10
        y <- 20
        z <- 30
        rm(list = c("x", "y"))
        stopifnot(!exists("x"))
        stopifnot(!exists("y"))
        stopifnot(exists("z"))
    "#,
    )
    .unwrap();
}

#[test]
fn rm_with_envir_argument() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("val", 99, envir = e)
        stopifnot(exists("val", envir = e))
        rm("val", envir = e)
        stopifnot(!exists("val", envir = e))
    "#,
    )
    .unwrap();
}

#[test]
fn rm_ignores_nonexistent_names() {
    let mut r = Session::new();
    // Should not error when removing a name that doesn't exist
    r.eval_source(r#"rm("nonexistent_var_xyz")"#).unwrap();
}

#[test]
fn remove_is_alias_for_rm() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- 42
        remove("x")
        stopifnot(!exists("x"))
    "#,
    )
    .unwrap();
}

#[test]
fn rm_list_equals_ls_clears_environment() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        a <- 1
        b <- 2
        c <- 3
        rm(list = ls())
        stopifnot(length(ls()) == 0)
    "#,
    )
    .unwrap();
}

// endregion

// region: local()

#[test]
fn local_evaluates_in_temporary_env() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        result <- local({
            temp_var <- 100
            temp_var + 1
        })
        stopifnot(result == 101)
        stopifnot(!exists("temp_var"))
    "#,
    )
    .unwrap();
}

#[test]
fn local_can_read_parent_bindings() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        x <- 10
        result <- local({
            x + 5
        })
        stopifnot(result == 15)
    "#,
    )
    .unwrap();
}

#[test]
fn local_does_not_leak_bindings() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        local({
            secret <- "hidden"
        })
        stopifnot(!exists("secret"))
    "#,
    )
    .unwrap();
}

#[test]
fn local_with_explicit_envir() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("val", 42, envir = e)
        result <- local(val, envir = e)
        stopifnot(result == 42)
    "#,
    )
    .unwrap();
}

#[test]
fn local_returns_last_expression() {
    let mut r = Session::new();
    let result = r
        .eval_source(
            r#"
        local({
            a <- 1
            b <- 2
            a + b
        })
    "#,
        )
        .unwrap()
        .value;
    assert_eq!(
        result.as_vector().and_then(|v| v.as_double_scalar()),
        Some(3.0)
    );
}

// endregion

// region: lockEnvironment() / environmentIsLocked()

#[test]
fn lock_environment_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        stopifnot(!environmentIsLocked(e))
        lockEnvironment(e)
        stopifnot(environmentIsLocked(e))
    "#,
    )
    .unwrap();
}

#[test]
fn environment_is_locked_returns_false_by_default() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        stopifnot(!environmentIsLocked(e))
    "#,
    )
    .unwrap();
}

// endregion

// region: lockBinding() / bindingIsLocked()

#[test]
fn lock_binding_basic() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("x", 10, envir = e)
        stopifnot(!bindingIsLocked("x", e))
        lockBinding("x", e)
        stopifnot(bindingIsLocked("x", e))
    "#,
    )
    .unwrap();
}

#[test]
fn binding_is_locked_returns_false_by_default() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("x", 10, envir = e)
        stopifnot(!bindingIsLocked("x", e))
    "#,
    )
    .unwrap();
}

#[test]
fn lock_environment_with_bindings_true() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("a", 1, envir = e)
        assign("b", 2, envir = e)
        lockEnvironment(e, bindings = TRUE)
        stopifnot(environmentIsLocked(e))
        stopifnot(bindingIsLocked("a", e))
        stopifnot(bindingIsLocked("b", e))
    "#,
    )
    .unwrap();
}

// endregion

// region: makeActiveBinding / isActiveBinding

#[test]
fn active_binding_basic_access() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        makeActiveBinding("x", function() 42, e)
        stopifnot(get("x", envir = e) == 42)
    "#,
    )
    .unwrap();
}

#[test]
fn active_binding_re_evaluates_on_each_access() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        counter <- 0L
        e <- new.env(parent = environment())
        makeActiveBinding("x", function() { counter <<- counter + 1L; counter }, e)
        a <- get("x", envir = e)
        b <- get("x", envir = e)
        c <- get("x", envir = e)
        stopifnot(a == 1L)
        stopifnot(b == 2L)
        stopifnot(c == 3L)
    "#,
    )
    .unwrap();
}

#[test]
fn active_binding_via_symbol_access() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        counter <- 0L
        makeActiveBinding("x", function() { counter <<- counter + 1L; counter }, environment())
        a <- x
        b <- x
        stopifnot(a == 1L)
        stopifnot(b == 2L)
    "#,
    )
    .unwrap();
}

#[test]
fn is_active_binding_true() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        makeActiveBinding("x", function() 42, e)
        stopifnot(isActiveBinding("x", e))
    "#,
    )
    .unwrap();
}

#[test]
fn is_active_binding_false_for_regular() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        assign("x", 42, envir = e)
        stopifnot(!isActiveBinding("x", e))
    "#,
    )
    .unwrap();
}

#[test]
fn active_binding_visible_in_ls() {
    let mut r = Session::new();
    r.eval_source(
        r#"
        e <- new.env(parent = emptyenv())
        makeActiveBinding("x", function() 42, e)
        stopifnot("x" %in% ls(envir = e))
    "#,
    )
    .unwrap();
}

// endregion
