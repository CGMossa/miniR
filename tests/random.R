# Test random number generation builtins

# set.seed for reproducibility
set.seed(42)
x <- runif(5)
cat("runif(5):", x, "\n")

# Same seed should give same results
set.seed(42)
y <- runif(5)
stopifnot(identical(x, y))
cat("PASS: set.seed reproducibility\n")

# rnorm
set.seed(1)
z <- rnorm(5)
cat("rnorm(5):", z, "\n")
stopifnot(length(z) == 5)
cat("PASS: rnorm\n")

# rnorm with parameters
z2 <- rnorm(3, mean = 10, sd = 2)
cat("rnorm(3, mean=10, sd=2):", z2, "\n")
stopifnot(length(z2) == 3)
cat("PASS: rnorm with params\n")

# runif with parameters
u <- runif(4, min = -1, max = 1)
cat("runif(4, -1, 1):", u, "\n")
stopifnot(all(u >= -1 & u <= 1))
cat("PASS: runif bounds\n")

# rbinom
b <- rbinom(5, size = 10, prob = 0.5)
cat("rbinom(5, 10, 0.5):", b, "\n")
stopifnot(length(b) == 5)
stopifnot(all(b >= 0 & b <= 10))
cat("PASS: rbinom\n")

# rpois
p <- rpois(5, lambda = 3)
cat("rpois(5, 3):", p, "\n")
stopifnot(length(p) == 5)
stopifnot(all(p >= 0))
cat("PASS: rpois\n")

# rexp
e <- rexp(5, rate = 2)
cat("rexp(5, 2):", e, "\n")
stopifnot(length(e) == 5)
stopifnot(all(e >= 0))
cat("PASS: rexp\n")

# sample from 1:10
set.seed(123)
s <- sample(10, 5)
cat("sample(10, 5):", s, "\n")
stopifnot(length(s) == 5)
stopifnot(all(s >= 1 & s <= 10))
stopifnot(length(unique(s)) == 5)  # no duplicates without replacement
cat("PASS: sample without replacement\n")

# sample with replacement
s2 <- sample(3, 10, replace = TRUE)
cat("sample(3, 10, replace=TRUE):", s2, "\n")
stopifnot(length(s2) == 10)
stopifnot(all(s2 >= 1 & s2 <= 3))
cat("PASS: sample with replacement\n")

# sample from a vector
set.seed(42)
v <- c("a", "b", "c", "d", "e")
s3 <- sample(v, 3)
cat("sample(c('a','b','c','d','e'), 3):", s3, "\n")
stopifnot(length(s3) == 3)
cat("PASS: sample from vector\n")

cat("\nAll random tests passed!\n")
