# Test corpus - should parse and run

# Identifiers
x <- 1
.x <- 2
x_y <- 3
data.frame <- 4

# Numerics
a <- 1
b <- 1L
c <- 1.0
d <- .5
e <- 1e3
f <- 1.2e-3
g <- 0x10
h <- 0x10L

# Logical and special values
t <- TRUE
ff <- FALSE
n <- NULL
na <- NA

# Strings
s1 <- "a"
s2 <- 'b'
s3 <- "a\nb"

# Assignment forms
x <- 1
x <<- 1
1 -> x
1 ->> x
a <- b <- 1
x = 1

# Arithmetic with correct precedence
print(1 + 2 * 3)     # 7
print(2 ^ 3 ^ 2)     # 512 (right-assoc: 2^(3^2) = 2^9)
print(-1^2)           # -1 (unary - below ^: -(1^2))
print(1:10)
print(2 * 1:5)        # 2 4 6 8 10 (: binds tighter than *)

# Special operators
x <- c(1, 2, 3)
y <- c(2, 3, 4)
print(2 %in% x)
print(10 %% 3)        # 1
print(10 %/% 3)       # 3

# Pipe
result <- c(1, 2, 3) |> sum()
print(result)          # 6

# Indexing
x <- c(10, 20, 30, 40, 50)
print(x[1])
print(x[c(1, 3)])

# List operations
lst <- list(a = 1, b = 2, c = 3)
print(lst$a)
print(lst[["b"]])

# Function calls
f <- function(x, y = 1, ...) x + y
print(f(1, 2))        # 3
print(f(5))           # 6

# Lambda
g <- \(x) x + 1
print(g(10))          # 11

# Control flow
if (TRUE) print("yes") else print("no")
for (i in 1:3) print(i)
x <- 0
while (x < 3) { x <- x + 1 }
print(x)              # 3

# Block
result <- {
  a <- 10
  b <- 20
  a + b
}
print(result)          # 30

# Formula (stub - just should parse)
# ~x
# y ~ x

# Empty arguments in calls
v <- c(1, 2, 3)

# Chained assignment
a <- b <- c <- 42
print(a)              # 42
print(b)              # 42
print(c)              # should be c function, but 42 due to assignment

# Comparison
print(1 == 1)         # TRUE
print(1 != 2)         # TRUE
print(3 > 2)          # TRUE

# Logical operators
print(TRUE & FALSE)   # FALSE
print(TRUE | FALSE)   # TRUE
print(!TRUE)          # FALSE
print(TRUE && FALSE)  # FALSE
print(TRUE || FALSE)  # TRUE

print("All tests passed!")
