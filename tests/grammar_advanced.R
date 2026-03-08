# Advanced grammar tests

# Namespace access (stub - just should parse)
# base::sum(1:10)
# base:::sum(1:10)

# Complex numbers (stub - treated as double)
z <- 1i
z2 <- 2.5i
z3 <- .5i
print(z)
print(z2)
print(z3)

# Hex numbers
print(0xFF)
print(0xFFp0)
print(0x1.fp3)

# DotDot (stub)
# ..1, ..2 reserved tokens should parse

# Empty arguments in calls
v <- c(1, 2, 3)
print(v)

# Formula (stub)
~x
y ~ x

# Chained postfix: lst$a
lst <- list(a = 1, b = list(c = 2))
print(lst$a)
print(lst$b)

# Slot (stub - treat like $)
# obj@slot

# T and F as identifiers (can be reassigned)
T <- 42
F <- 99
print(T)
print(F)
# But TRUE/FALSE are literals, not reassignable
print(TRUE)
print(FALSE)

# ** as power operator
print(2 ** 3)

# Return from function
f <- function(x) {
  if (x > 0) return(x)
  return(-x)
}
print(f(5))
print(f(-3))

# Repeat with break
x <- 0
repeat {
  x <- x + 1
  if (x >= 3) break
}
print(x)

# Next in for loop
total <- 0
for (i in 1:5) {
  if (i == 3) next
  total <- total + i
}
print(total)

# Nested function calls
print(sum(c(1, 2, 3, 4, 5)))

# Negative indexing
v <- c(10, 20, 30, 40, 50)
print(v[-1])
print(v[c(-1, -2)])

# Logical indexing
v <- c(10, 20, 30, 40, 50)
print(v[v > 25])

# While loop with comparison
x <- 10
while (x > 0) x <- x - 2
print(x)

# Multiple expressions in block
result <- {
  a <- 1
  b <- 2
  c <- 3
  a + b + c
}
print(result)

print("Advanced tests passed!")
