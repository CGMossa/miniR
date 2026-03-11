# Test on.exit()

# Basic on.exit
cleanup <- NULL
f <- function() {
  on.exit(cleanup <<- "ran")
  42
}
result <- f()
stopifnot(result == 42)
stopifnot(cleanup == "ran")
cat("PASS: basic on.exit\n")

# on.exit runs even when error occurs
cleanup2 <- NULL
tryCatch(
  {
    g <- function() {
      on.exit(cleanup2 <<- "error_cleanup")
      stop("oops")
    }
    g()
  },
  error = function(e) NULL
)
stopifnot(cleanup2 == "error_cleanup")
cat("PASS: on.exit runs on error\n")

# on.exit with add = TRUE
log <- character(0)
h <- function() {
  on.exit(log <<- c(log, "first"))
  on.exit(log <<- c(log, "second"), add = TRUE)
  "done"
}
result <- h()
stopifnot(result == "done")
stopifnot(length(log) == 2)
stopifnot(log[1] == "first")
stopifnot(log[2] == "second")
cat("PASS: on.exit with add = TRUE\n")

# on.exit without add replaces
log2 <- character(0)
i <- function() {
  on.exit(log2 <<- c(log2, "first"))
  on.exit(log2 <<- c(log2, "replaced"))
  "done"
}
i()
stopifnot(length(log2) == 1)
stopifnot(log2[1] == "replaced")
cat("PASS: on.exit without add replaces\n")

# on.exit() with no args clears
log3 <- character(0)
j <- function() {
  on.exit(log3 <<- c(log3, "should not run"))
  on.exit()  # clears
  "done"
}
j()
stopifnot(length(log3) == 0)
cat("PASS: on.exit() clears handlers\n")

# on.exit with return()
cleanup3 <- NULL
k <- function() {
  on.exit(cleanup3 <<- "return_cleanup")
  return(99)
}
result <- k()
stopifnot(result == 99)
stopifnot(cleanup3 == "return_cleanup")
cat("PASS: on.exit runs with return()\n")

cat("\nAll on.exit tests passed!\n")
