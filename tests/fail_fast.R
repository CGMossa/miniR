# Test fail_fast argument for apply-family functions
# Default: fail_fast = FALSE (collect all errors)
# When TRUE: stop at first error (legacy behavior)

# Helper that errors on even numbers
err_on_even <- function(x) {
  if (x %% 2 == 0) stop(paste("even:", x))
  x * 10
}

# --- sapply ---

# fail_fast = TRUE: stops at first error
res <- tryCatch(sapply(1:4, err_on_even, fail_fast = TRUE), error = function(e) e)
stopifnot(inherits(res, "error"))
cat("PASS: sapply fail_fast=TRUE stops at first error\n")

# fail_fast = FALSE (default): collects all errors
res <- sapply(1:5, err_on_even)
# Should have results for odd numbers, errors for even
stopifnot(length(res) == 5)
cat("PASS: sapply fail_fast=FALSE collects all results\n")

# --- lapply ---

res <- lapply(1:5, err_on_even)
stopifnot(length(res) == 5)
# Odd indices should be successful
stopifnot(res[[1]] == 10)
stopifnot(res[[3]] == 30)
stopifnot(res[[5]] == 50)
cat("PASS: lapply fail_fast=FALSE collects all results\n")

res <- tryCatch(lapply(1:4, err_on_even, fail_fast = TRUE), error = function(e) e)
stopifnot(inherits(res, "error"))
cat("PASS: lapply fail_fast=TRUE stops at first error\n")

# --- Reduce ---

bad_add <- function(a, b) {
  if (b == 3) stop("bad value")
  a + b
}

res <- tryCatch(Reduce(bad_add, 1:5, fail_fast = TRUE), error = function(e) e)
stopifnot(inherits(res, "error"))
cat("PASS: Reduce fail_fast=TRUE stops at first error\n")

# --- Filter ---

bad_pred <- function(x) {
  if (x == 3) stop("bad")
  x > 2
}

res <- Filter(bad_pred, 1:5)
# Should still include elements where predicate succeeded and was TRUE
stopifnot(4 %in% res)
stopifnot(5 %in% res)
cat("PASS: Filter fail_fast=FALSE skips errors\n")

# --- Map ---

bad_mul <- function(x, y) {
  if (x == 2) stop("bad")
  x * y
}

res <- Map(bad_mul, 1:3, 10:12)
stopifnot(length(res) == 3)
stopifnot(res[[1]] == 10)
stopifnot(res[[3]] == 36)
cat("PASS: Map fail_fast=FALSE collects all results\n")

# --- apply ---

m <- matrix(c(1, 2, 3, 4, 5, 6), nrow = 2)
res <- apply(m, 1, err_on_even)
# Row 1 has values 1,3,5 (all odd) — should work
# Row 2 has values 2,4,6 (all even) — should error
cat("PASS: apply fail_fast=FALSE collects results\n")

cat("\nAll fail_fast tests passed!\n")
