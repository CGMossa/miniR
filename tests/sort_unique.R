# Test sort_unique() — miniR extension: single-pass sorted unique via BTreeSet

# Basic integer
x <- c(3L, 1L, 4L, 1L, 5L, 9L, 2L, 6L, 5L, 3L)
result <- sort_unique(x)
stopifnot(identical(result, c(1L, 2L, 3L, 4L, 5L, 6L, 9L)))
cat("PASS: integer sort_unique\n")

# Decreasing
result <- sort_unique(x, decreasing = TRUE)
stopifnot(identical(result, c(9L, 6L, 5L, 4L, 3L, 2L, 1L)))
cat("PASS: integer sort_unique decreasing\n")

# Double
y <- c(3.1, 1.4, 1.4, 2.7, 3.1, 0.5)
result <- sort_unique(y)
stopifnot(identical(result, c(0.5, 1.4, 2.7, 3.1)))
cat("PASS: double sort_unique\n")

# Negative doubles
z <- c(-1.0, 2.0, -3.0, 2.0, -1.0)
result <- sort_unique(z)
stopifnot(identical(result, c(-3.0, -1.0, 2.0)))
cat("PASS: negative double sort_unique\n")

# Character
s <- c("banana", "apple", "cherry", "apple", "banana")
result <- sort_unique(s)
stopifnot(identical(result, c("apple", "banana", "cherry")))
cat("PASS: character sort_unique\n")

# Equivalence with sort(unique())
big <- c(5L, 3L, 8L, 1L, 3L, 5L, 7L, 2L, 8L)
stopifnot(identical(sort_unique(big), sort(unique(big))))
cat("PASS: sort_unique matches sort(unique())\n")

# NA handling — NAs should sort last
w <- c(3L, NA, 1L, NA, 2L)
result <- sort_unique(w)
stopifnot(identical(result, c(1L, 2L, 3L, NA)))
cat("PASS: integer NA sorts last\n")

cat("\nAll sort_unique tests passed!\n")
