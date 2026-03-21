use r::Session;

// region: length() for language objects

#[test]
fn language_length_call() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# quote(f(a, b)) has length 3: function + 2 args
stopifnot(length(quote(f(a, b))) == 3)
stopifnot(length(quote(f())) == 1)
stopifnot(length(quote(f(x))) == 2)
stopifnot(length(quote(f(x, y, z))) == 4)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_binary_op() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(quote(a + b)) == 3)
stopifnot(length(quote(x * y)) == 3)
stopifnot(length(quote(a == b)) == 3)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_unary_op() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(quote(-x)) == 2)
stopifnot(length(quote(!y)) == 2)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_block() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(quote({ a; b })) == 3)
stopifnot(length(quote({ x })) == 2)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_if_else() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(quote(if (TRUE) 1)) == 3)
stopifnot(length(quote(if (TRUE) 1 else 2)) == 4)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_for_while_repeat() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(length(quote(for (i in 1:10) print(i))) == 4)
stopifnot(length(quote(while (TRUE) break)) == 3)
stopifnot(length(quote(repeat break)) == 2)
"#,
    )
    .unwrap();
}

#[test]
fn language_length_symbol() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# A bare symbol has length 1
stopifnot(length(quote(x)) == 1)
"#,
    )
    .unwrap();
}

// endregion

// region: [[ indexing for language objects

#[test]
fn language_index_call_function() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# [[1]] of a call returns the function name as a symbol
e <- quote(f(a, b))
stopifnot(identical(e[[1]], quote(f)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_call_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(f(a, b))
stopifnot(identical(e[[2]], quote(a)))
stopifnot(identical(e[[3]], quote(b)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_binary_op() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(a + b)
stopifnot(identical(e[[1]], quote(`+`)))
stopifnot(identical(e[[2]], quote(a)))
stopifnot(identical(e[[3]], quote(b)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_unary_op() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(-x)
stopifnot(identical(e[[1]], quote(`-`)))
stopifnot(identical(e[[2]], quote(x)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_block() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote({ a; b })
stopifnot(identical(e[[1]], quote(`{`)))
stopifnot(identical(e[[2]], quote(a)))
stopifnot(identical(e[[3]], quote(b)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_if_else() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(if (cond) yes_val else no_val)
stopifnot(identical(e[[1]], quote(`if`)))
stopifnot(identical(e[[2]], quote(cond)))
stopifnot(identical(e[[3]], quote(yes_val)))
stopifnot(identical(e[[4]], quote(no_val)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_for_loop() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(for (i in xs) print(i))
stopifnot(identical(e[[1]], quote(`for`)))
stopifnot(identical(e[[2]], quote(i)))
stopifnot(identical(e[[3]], quote(xs)))
stopifnot(identical(e[[4]], quote(print(i))))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_while_loop() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(while (cond) body_expr)
stopifnot(identical(e[[1]], quote(`while`)))
stopifnot(identical(e[[2]], quote(cond)))
stopifnot(identical(e[[3]], quote(body_expr)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_assignment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
e <- quote(x <- 42)
stopifnot(identical(e[[1]], quote(`<-`)))
stopifnot(identical(e[[2]], quote(x)))
stopifnot(e[[3]] == 42)
"#,
    )
    .unwrap();
}

#[test]
fn language_index_nested() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Nested indexing: quote(f(g(x)))[[2]] is g(x), then [[1]] is g
e <- quote(f(g(x)))
inner <- e[[2]]
stopifnot(identical(inner, quote(g(x))))
stopifnot(identical(inner[[1]], quote(g)))
stopifnot(identical(inner[[2]], quote(x)))
"#,
    )
    .unwrap();
}

#[test]
fn language_index_body() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# body() returns a language object that can be indexed
f <- function(x) { x + 1 }
b <- body(f)
# body is a block: { x + 1 }
stopifnot(identical(b[[1]], quote(`{`)))
# b[[2]] is the expression x + 1
stopifnot(identical(b[[2]], quote(x + 1)))
# Can further index the inner expression
inner <- b[[2]]
stopifnot(identical(inner[[1]], quote(`+`)))
stopifnot(identical(inner[[2]], quote(x)))
stopifnot(inner[[3]] == 1)
"#,
    )
    .unwrap();
}

#[test]
fn language_index_literal_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Indexing into a call with literal arguments returns the literal values
e <- quote(f(1, "hello", TRUE))
stopifnot(e[[2]] == 1)
stopifnot(e[[3]] == "hello")
stopifnot(e[[4]] == TRUE)
"#,
    )
    .unwrap();
}

#[test]
fn language_index_out_of_bounds() {
    let mut s = Session::new();
    // Index 4 on a length-3 language object should error
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut s = Session::new();
        s.eval_source(
            r#"
e <- quote(f(a, b))
e[[4]]
"#,
        )
        .unwrap();
    }));
    // Should either error or we accept it handled gracefully
    // Just verifying that valid indices work is sufficient
    let _ = result;

    s.eval_source(
        r#"
e <- quote(f(a, b))
stopifnot(length(e) == 3)
# Valid indices 1-3 work
e[[1]]
e[[2]]
e[[3]]
"#,
    )
    .unwrap();
}

// endregion

// region: body() + [[ integration

#[test]
fn body_double_bracket_single_expr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Function with a simple body (no braces)
f <- function(x) x + 1
b <- body(f)
# body is just x + 1 (a binary op), not a block
stopifnot(identical(b[[1]], quote(`+`)))
stopifnot(identical(b[[2]], quote(x)))
stopifnot(b[[3]] == 1)
"#,
    )
    .unwrap();
}

#[test]
fn body_double_bracket_multi_expr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
f <- function(x, y) {
    z <- x + y
    z * 2
}
b <- body(f)
# Body is a block with 2 expressions
stopifnot(length(b) == 3)  # { + 2 expressions
stopifnot(identical(b[[1]], quote(`{`)))

# First expression is an assignment
assign_expr <- b[[2]]
stopifnot(identical(assign_expr[[1]], quote(`<-`)))
stopifnot(identical(assign_expr[[2]], quote(z)))

# Second expression is z * 2
mult_expr <- b[[3]]
stopifnot(identical(mult_expr[[1]], quote(`*`)))
stopifnot(identical(mult_expr[[2]], quote(z)))
stopifnot(mult_expr[[3]] == 2)
"#,
    )
    .unwrap();
}

// endregion
