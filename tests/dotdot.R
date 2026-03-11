# Test ... (dots) and ..1, ..2 argument access

# Basic ... forwarding
f <- function(...) paste(...)
result <- f("a", "b", "c")
stopifnot(result == "a b c")
cat("PASS: basic ... forwarding\n")

# ..1, ..2 access
g <- function(...) ..1
result <- g("first", "second", "third")
stopifnot(result == "first")
cat("PASS: ..1 access\n")

h <- function(...) ..2
result <- h("first", "second", "third")
stopifnot(result == "second")
cat("PASS: ..2 access\n")

# ..3 access
i <- function(...) ..3
result <- i("a", "b", "c")
stopifnot(result == "c")
cat("PASS: ..3 access\n")

# ... in nested calls
outer <- function(...) inner(...)
inner <- function(x, y) x + y
result <- outer(10, 20)
stopifnot(result == 30)
cat("PASS: ... forwarding to inner function\n")

# ... with named arguments
make_list <- function(...) list(...)
result <- make_list(a = 1, b = 2)
stopifnot(result$a == 1)
stopifnot(result$b == 2)
cat("PASS: ... with named arguments\n")

# ... passes unmatched named args
f2 <- function(x, ...) list(...)
result <- f2(x = 1, y = 2, z = 3)
stopifnot(result$y == 2)
stopifnot(result$z == 3)
cat("PASS: unmatched named args go to ...\n")

# ..1 with various types
f3 <- function(...) ..1 * 2
stopifnot(f3(5) == 10)
cat("PASS: ..1 with computation\n")

# length of ...
f4 <- function(...) length(list(...))
stopifnot(f4(1, 2, 3) == 3)
stopifnot(f4() == 0)
cat("PASS: length of ...\n")

cat("\nAll dotdot tests passed!\n")
