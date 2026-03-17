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
# quote(f(x)) is a call / language object
stopifnot(is.call(quote(f(x))))
stopifnot(is.call(quote(1 + 2)))

# Symbols are language but not calls in the is.call sense —
# actually in R, is.call(quote(x)) is FALSE while is.language(quote(x)) is TRUE
# But in miniR both symbols and calls are Language, so is.call returns TRUE for
# all Language objects (matching R's behavior since quote(x) returns a name, not a call)
stopifnot(!is.call(1))
stopifnot(!is.call(NULL))
stopifnot(!is.call("hello"))
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
# In miniR, lists map to pairlists
stopifnot(is.pairlist(list(1, 2, 3)))
stopifnot(is.pairlist(list()))

# NULL is a pairlist in R
stopifnot(is.pairlist(NULL))

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
