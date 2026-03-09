# Test metaprogramming builtins: formals, body, args, call, expression, Recall

# === formals() ===
f <- function(x, y = 10, ...) x + y
result <- formals(f)
cat("formals(f): ")
print(result)
# Should be a list with names "x", "y", "..."

# formals of no-arg function
g <- function() 42
cat("formals(g): ")
print(formals(g))
# Should be NULL

# formals of builtin
cat("formals(print): ")
print(formals(print))
# Should be NULL (builtin)

# === body() ===
cat("body(f): ")
print(body(f))
# Should print x + y

cat("body(g): ")
print(body(g))
# Should print 42

cat("body(print): ")
print(body(print))
# Should be NULL (builtin)

# === args() ===
cat("args(f): ")
print(args(f))
# Should be same as formals(f)

# === call() ===
cl <- call("sum", 1, 2, 3)
cat("call('sum', 1, 2, 3): ")
print(cl)
# Should print sum(1, 2, 3)

# Evaluate the call
cat("eval(call('sum', 1, 2, 3)): ")
print(eval(cl))
# Should print 6

# call with named args
cl2 <- call("paste", "hello", "world", sep = "-")
cat("call with named: ")
print(cl2)

# === expression() ===
e <- expression(1 + 2, 3 * 4)
cat("expression(1+2, 3*4): ")
print(e)

# === formals with defaults ===
h <- function(a, b = 5, c = a + b) a * b * c
cat("formals(h): ")
print(formals(h))

cat("DONE\n")
