use r::Session;

#[test]
fn tilde_builds_formula_objects() {
    let mut s = Session::new();
    s.eval_source(
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
    )
    .expect("formula tests failed");
}
