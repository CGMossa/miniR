use r::Session;

#[test]
fn three_pass_argument_matching() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# Exact matching
f <- function(alpha, beta) c(alpha, beta)
stopifnot(identical(f(alpha = 1, beta = 2), c(1, 2)))

# Partial matching
stopifnot(identical(f(al = 1, b = 2), c(1, 2)))

# Exact takes priority over partial
g <- function(abc, ab) c(abc, ab)
stopifnot(identical(g(ab = 10, abc = 20), c(20, 10)))

# Positional fill after named match
h <- function(x, y, z) c(x, y, z)
stopifnot(identical(h(y = 20, 10, 30), c(10, 20, 30)))

# Dots absorb extras
d <- function(x, ...) list(x = x, dots = list(...))
r <- d(1, extra = 99)
stopifnot(r$x == 1)

# Unused argument error (no ...)
tryCatch(
  { f2 <- function(x) x; f2(gamma = 1, 2) },
  error = function(e) stopifnot(grepl("unused", conditionMessage(e)))
)

# Extra positional error
tryCatch(
  { f3 <- function(x) x; f3(1, 2) },
  error = function(e) stopifnot(grepl("unused", conditionMessage(e)))
)

# Ambiguous partial error
tryCatch(
  { f4 <- function(alpha, also) 1; f4(al = 1) },
  error = function(e) stopifnot(grepl("matches multiple", conditionMessage(e)))
)

# Defaults still work when not supplied
f5 <- function(x, y = 10) x + y
stopifnot(f5(1) == 11)
stopifnot(f5(1, y = 20) == 21)
"#,
    )
    .expect("argument matching tests failed");
}
