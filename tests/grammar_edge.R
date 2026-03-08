# Edge cases that should parse correctly

# Backtick identifiers
`my var` <- 10
print(`my var`)

# Dotted identifier starting with .
.hidden <- "secret"
print(.hidden)

# Chained assignment
a <- b <- c <- 100
print(a)
print(b)

# Nested if/else
x <- 5
result <- if (x > 10) "big" else if (x > 3) "medium" else "small"
print(result)

# Function with default args
f <- function(x, y = 10, z = 20) x + y + z
print(f(1))
print(f(1, 2))
print(f(1, 2, 3))

# Deeply nested arithmetic
print(((1 + 2) * 3 - 4) / 5)

# String escapes
s <- "hello\tworld\n"
print(s)

# Integer suffix
x <- 42L
print(x)

# Scientific notation
print(1e10)
print(1.5e-3)

# Chained indexing
m <- list(a = c(10, 20, 30))
print(m$a[2])

# Pipe chain
result <- c(1, 2, 3, 4, 5) |> sum()
print(result)

# Boolean operations mixed
print(TRUE & !FALSE)
print(!(TRUE & FALSE))

# Range and arithmetic
print(sum(1:100))

# Comparison chain
x <- 5
print(x > 3 & x < 10)

print("Edge cases passed!")
