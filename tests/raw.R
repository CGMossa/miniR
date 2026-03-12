# Test raw vector builtins

# raw(n) creates zero-filled byte vector
x <- raw(5)
stopifnot(length(x) == 5)
stopifnot(all(x == 0))
cat("PASS: raw(n) creates zero vector\n")

# raw(0) gives empty vector
stopifnot(length(raw(0)) == 0)
cat("PASS: raw(0) gives empty vector\n")

# charToRaw / rawToChar roundtrip
y <- charToRaw("Hello")
stopifnot(length(y) == 5)
stopifnot(y[1] == 72)  # 'H'
stopifnot(rawToChar(y) == "Hello")
cat("PASS: charToRaw/rawToChar roundtrip\n")

# rawShift left
z <- charToRaw("A")  # 65
stopifnot(rawShift(z, 1) == 130)  # 65 << 1
cat("PASS: rawShift left\n")

# rawShift right
stopifnot(rawShift(z, -1) == 32)  # 65 >> 1
cat("PASS: rawShift right\n")

# rawShift vectorized
abc <- charToRaw("ABC")  # 65, 66, 67
shifted <- rawShift(abc, 1)
stopifnot(shifted[1] == 130)
stopifnot(shifted[2] == 132)
stopifnot(shifted[3] == 134)
cat("PASS: rawShift vectorized\n")

# as.raw truncates to lowest byte
stopifnot(as.raw(256) == 0)
stopifnot(as.raw(257) == 1)
stopifnot(as.raw(65) == 65)
cat("PASS: as.raw truncation\n")

# raw(n) error on negative
err <- tryCatch(raw(-1), error = function(e) conditionMessage(e))
stopifnot(grepl("invalid", err))
cat("PASS: raw(-1) gives error\n")

# rawShift error on out-of-range shift
err <- tryCatch(rawShift(raw(1), 9), error = function(e) conditionMessage(e))
stopifnot(grepl("between -8 and 8", err))
cat("PASS: rawShift out-of-range error\n")

cat("\nAll raw tests passed!\n")
