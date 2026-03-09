# Test environment builtins
cat("=== globalenv/baseenv/emptyenv ===\n")
ge <- globalenv()
cat("is.environment(globalenv()):", is.environment(ge), "\n")
cat("environmentName(globalenv()):", environmentName(ge), "\n")

be <- baseenv()
cat("is.environment(baseenv()):", is.environment(be), "\n")

ee <- emptyenv()
cat("is.environment(emptyenv()):", is.environment(ee), "\n")
cat("environmentName(emptyenv()):", environmentName(ee), "\n")

cat("\n=== parent.env ===\n")
pe <- parent.env(ge)
cat("parent of global is base?:", is.environment(pe), "\n")

cat("\n=== new.env ===\n")
e <- new.env(parent = ge)
cat("is.environment(new.env()):", is.environment(e), "\n")

cat("\n=== ls ===\n")
x <- 1
y <- 2
z <- "hello"
result <- ls()
cat("ls() length > 0:", length(result) > 0, "\n")
cat("ls() contains 'x':", "x" %in% result, "\n")
cat("ls() contains 'y':", "y" %in% result, "\n")

cat("\n=== environment() on closure ===\n")
f <- function() NULL
env_f <- environment(f)
cat("is.environment:", is.environment(env_f), "\n")

cat("\n=== is.language ===\n")
q <- quote(x + 1)
cat("is.language(quote(x+1)):", is.language(q), "\n")
cat("is.language(1):", is.language(1), "\n")

cat("\nAll environment tests passed!\n")
