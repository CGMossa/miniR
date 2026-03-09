# Test data frame operations

df <- data.frame(name = c("Alice", "Bob", "Charlie"), age = c(30, 25, 35), score = c(95.5, 87.2, 91.8))

# Basic properties
stopifnot(nrow(df) == 3)
stopifnot(ncol(df) == 3)
stopifnot(identical(names(df), c("name", "age", "score")))
stopifnot(is.data.frame(df))

# Column access
stopifnot(identical(df$age, c(30, 25, 35)))
stopifnot(identical(df[["name"]], c("Alice", "Bob", "Charlie")))

# Single column selection (drop=TRUE)
ages <- df[, "age"]
stopifnot(identical(ages, c(30, 25, 35)))

# Logical row filtering
young <- df[df$age < 32, ]
stopifnot(nrow(young) == 2)
stopifnot(identical(young$name, c("Alice", "Bob")))

# Integer row selection
first <- df[1:2, ]
stopifnot(nrow(first) == 2)
stopifnot(identical(first$name, c("Alice", "Bob")))

# All columns, all rows
full <- df[, ]
stopifnot(nrow(full) == 3)
stopifnot(ncol(full) == 3)

cat("All data frame tests passed!\n")
