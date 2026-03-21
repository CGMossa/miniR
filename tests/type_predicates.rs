use r::Session;

/// Tests for is.* type predicate builtins.
#[test]
fn is_null() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.null(NULL))
stopifnot(!is.null(1))
stopifnot(!is.null(list()))
stopifnot(!is.null(""))
"#,
    )
    .unwrap();
}

#[test]
#[ignore = "ordered() constructor not yet implemented"]
fn is_ordered() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Unordered factor
f <- factor(c("a", "b", "c"))
stopifnot(!is.ordered(f))

# Ordered factor
of <- ordered(c("low", "med", "high"), levels = c("low", "med", "high"))
stopifnot(is.ordered(of))

# Not a factor at all
stopifnot(!is.ordered(1))
stopifnot(!is.ordered(NULL))
stopifnot(!is.ordered("abc"))
"#,
    )
    .unwrap();
}

#[test]
fn is_call() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# quote(f(x)) is a call — function call expression
stopifnot(is.call(quote(f(x))))
# Binary ops are calls in R (calls to the operator)
stopifnot(is.call(quote(1 + 2)))

# Symbols are NOT calls — is.call(quote(x)) is FALSE in R
stopifnot(!is.call(quote(x)))

# is.language is TRUE for both calls and symbols
stopifnot(is.language(quote(f(x))))
stopifnot(is.language(quote(x)))

# Non-language objects
stopifnot(!is.call(1))
stopifnot(!is.call(NULL))
stopifnot(!is.call("hello"))
stopifnot(!is.call(list()))
"#,
    )
    .unwrap();
}

#[test]
fn is_symbol_and_is_name() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# quote(x) produces a symbol
stopifnot(is.symbol(quote(x)))
stopifnot(is.name(quote(x)))

# quote(f(x)) is a call, not a symbol
stopifnot(!is.symbol(quote(f(x))))
stopifnot(!is.name(quote(f(x))))

# Non-language objects
stopifnot(!is.symbol(1))
stopifnot(!is.symbol(NULL))
stopifnot(!is.name("hello"))
"#,
    )
    .unwrap();
}

#[test]
fn is_expression() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# expression() creates expression objects
e <- expression(1 + 2)
stopifnot(is.expression(e))

# Regular values are not expressions
stopifnot(!is.expression(1))
stopifnot(!is.expression(NULL))
stopifnot(!is.expression(list(1, 2)))
stopifnot(!is.expression(quote(x)))
"#,
    )
    .unwrap();
}

#[test]
fn is_pairlist() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# NULL is a pairlist in R (it's the empty pairlist)
stopifnot(is.pairlist(NULL))

# Lists are NOT pairlists in R (they are VECSXP, not LISTSXP)
stopifnot(!is.pairlist(list(1, 2, 3)))
stopifnot(!is.pairlist(list()))

# Vectors are not pairlists
stopifnot(!is.pairlist(1))
stopifnot(!is.pairlist(c(1, 2)))
stopifnot(!is.pairlist("hello"))
"#,
    )
    .unwrap();
}

#[test]
fn is_primitive_vs_is_function() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Builtins are primitive
stopifnot(is.primitive(sum))
stopifnot(is.primitive(c))
stopifnot(is.primitive(length))

# User-defined closures are NOT primitive
f <- function(x) x + 1
stopifnot(!is.primitive(f))

# Both builtins and closures are functions
stopifnot(is.function(sum))
stopifnot(is.function(f))

# Non-functions are neither
stopifnot(!is.primitive(1))
stopifnot(!is.function(1))
stopifnot(!is.primitive(NULL))
"#,
    )
    .unwrap();
}

#[test]
fn is_array_with_dim() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# A vector with dim attribute is an array
x <- 1:6
dim(x) <- c(2, 3)
stopifnot(is.array(x))

# A matrix is also an array
m <- matrix(1:4, nrow = 2)
stopifnot(is.array(m))

# A 3D array
a <- array(1:24, dim = c(2, 3, 4))
stopifnot(is.array(a))

# Plain vectors without dim are NOT arrays
stopifnot(!is.array(1:6))
stopifnot(!is.array(c(1, 2, 3)))
stopifnot(!is.array(NULL))
stopifnot(!is.array(list(1, 2)))
"#,
    )
    .unwrap();
}

#[test]
fn grepl_na_handling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# grepl returns NA for NA inputs
result <- grepl("a", c("abc", NA, "def"))
stopifnot(identical(result, c(TRUE, NA, FALSE)))
"#,
    )
    .unwrap();
}

#[test]
fn grep_na_handling() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# grep skips NA elements (returns indices of matches, excluding NAs)
result <- grep("a", c("abc", NA, "def"))
stopifnot(identical(result, 1L))

# grep with value=TRUE also skips NAs
result_val <- grep("a", c("abc", NA, "xyz"), value = TRUE)
stopifnot(identical(result_val, "abc"))
"#,
    )
    .unwrap();
}

#[test]
fn is_recursive() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Lists are recursive
stopifnot(is.recursive(list(1, 2, 3)))
stopifnot(is.recursive(list()))

# Environments are recursive
stopifnot(is.recursive(environment()))

# Language objects (calls) are recursive
stopifnot(is.recursive(quote(f(x))))
stopifnot(is.recursive(quote(1 + 2)))

# Symbols are NOT recursive
stopifnot(!is.recursive(quote(x)))

# Atomic vectors and NULL are NOT recursive
stopifnot(!is.recursive(1:3))
stopifnot(!is.recursive(c(1.0, 2.0)))
stopifnot(!is.recursive("hello"))
stopifnot(!is.recursive(TRUE))
stopifnot(!is.recursive(NULL))
"#,
    )
    .unwrap();
}

#[test]
fn is_atomic() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Atomic vectors are atomic
stopifnot(is.atomic(1:3))
stopifnot(is.atomic(c(1.0, 2.0)))
stopifnot(is.atomic("hello"))
stopifnot(is.atomic(TRUE))
stopifnot(is.atomic(1i))

# NULL is atomic
stopifnot(is.atomic(NULL))

# Lists are NOT atomic
stopifnot(!is.atomic(list(1, 2)))

# Language objects are NOT atomic
stopifnot(!is.atomic(quote(f(x))))
stopifnot(!is.atomic(quote(x)))

# Environments are NOT atomic
stopifnot(!is.atomic(environment()))

# Functions are NOT atomic
stopifnot(!is.atomic(sum))
"#,
    )
    .unwrap();
}

#[test]
fn is_finite_infinite_nan() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# is.finite: TRUE for finite numbers, FALSE for Inf/-Inf/NaN/NA
stopifnot(identical(is.finite(c(1, Inf, -Inf, NaN, NA)), c(TRUE, FALSE, FALSE, FALSE, FALSE)))
stopifnot(identical(is.finite(1:3), c(TRUE, TRUE, TRUE)))
stopifnot(identical(is.finite(c(1L, NA_integer_)), c(TRUE, FALSE)))

# is.infinite: TRUE only for Inf and -Inf
stopifnot(identical(is.infinite(c(1, Inf, -Inf, NaN, NA)), c(FALSE, TRUE, TRUE, FALSE, FALSE)))
stopifnot(identical(is.infinite(1:3), c(FALSE, FALSE, FALSE)))

# is.nan: TRUE only for NaN, FALSE for NA
stopifnot(identical(is.nan(c(1, Inf, -Inf, NaN, NA)), c(FALSE, FALSE, FALSE, TRUE, FALSE)))
stopifnot(identical(is.nan(1:3), c(FALSE, FALSE, FALSE)))

# Logical vectors with is.finite
stopifnot(identical(is.finite(c(TRUE, FALSE, NA)), c(TRUE, TRUE, FALSE)))
"#,
    )
    .unwrap();
}

#[test]
fn is_numeric_excludes_factors() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Numeric types
stopifnot(is.numeric(1))
stopifnot(is.numeric(1.5))
stopifnot(is.numeric(1L))
stopifnot(is.numeric(1:3))

# NOT numeric
stopifnot(!is.numeric("abc"))
stopifnot(!is.numeric(TRUE))
stopifnot(!is.numeric(NULL))
stopifnot(!is.numeric(list()))

# Factors are NOT numeric despite integer storage
f <- factor(c("a", "b", "c"))
stopifnot(!is.numeric(f))
"#,
    )
    .unwrap();
}

#[test]
fn is_language_for_calls_and_symbols() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Both calls and symbols are language objects
stopifnot(is.language(quote(f(x))))
stopifnot(is.language(quote(x)))
stopifnot(is.language(quote(1 + 2)))

# Non-language objects
stopifnot(!is.language(1))
stopifnot(!is.language("hello"))
stopifnot(!is.language(NULL))
stopifnot(!is.language(list()))
stopifnot(!is.language(TRUE))
"#,
    )
    .unwrap();
}

#[test]
fn is_environment() {
    let mut s = Session::new();
    s.eval_source(
        r#"
stopifnot(is.environment(environment()))
stopifnot(is.environment(globalenv()))
stopifnot(is.environment(baseenv()))
stopifnot(!is.environment(1))
stopifnot(!is.environment(NULL))
stopifnot(!is.environment(list()))
"#,
    )
    .unwrap();
}
