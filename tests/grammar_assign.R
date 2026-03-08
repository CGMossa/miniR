# Assignment grammar tests

# Basic right-assign
42 -> x
print(x)              # 42

# Right super-assign from within a function
f <- function() {
  99 ->> outer_var
}
f()
print(outer_var)      # 99

# Chained right-assign: 5 -> c -> d means (5 -> c) -> d
5 -> a -> b
print(a)              # 5
print(b)              # 5

# Chained left-assign: a <- b <- 10 means a <- (b <- 10)
a <- b <- 10
print(a)              # 10
print(b)              # 10

# Right-assign with expression
1 + 2 -> result
print(result)         # 3

# Right super-assign with expression
g <- function() {
  100 + 1 ->> outer_val
}
g()
print(outer_val)      # 101

# Equals assignment
x = 42
print(x)              # 42

# Mixed assignment styles
a <- 1
2 -> b
c = 3
print(a + b + c)      # 6

# Assignment to index targets
v <- c(10, 20, 30)
v[1] <- 99
print(v[1])           # 99

# Assignment to list element
lst <- list(a = 1, b = 2)
lst$a <- 42
print(lst$a)          # 42

# Assignment to double-bracket
lst[["b"]] <- 99
print(lst[["b"]])     # 99

# Chained right-assign three deep
10 -> p -> q -> r
print(p)              # 10
print(q)              # 10
print(r)              # 10

print("Assignment tests passed!")
