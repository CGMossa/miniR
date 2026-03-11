# Test basic complex number support

# Complex literal
z <- 3+4i
stopifnot(is.complex(z))
cat("PASS: complex literal is complex\n")

# Re and Im
stopifnot(Re(z) == 3)
stopifnot(Im(z) == 4)
cat("PASS: Re and Im\n")

# Mod (absolute value)
stopifnot(Mod(z) == 5)
cat("PASS: Mod\n")

# Arg
stopifnot(abs(Arg(1+0i)) < 1e-10)
stopifnot(abs(Arg(-1+0i) - pi) < 1e-10)
cat("PASS: Arg\n")

# Conj
z2 <- Conj(z)
stopifnot(Re(z2) == 3)
stopifnot(Im(z2) == -4)
cat("PASS: Conj\n")

# complex() constructor
z3 <- complex(real = 1, imaginary = 2)
stopifnot(Re(z3) == 1)
stopifnot(Im(z3) == 2)
cat("PASS: complex() constructor\n")

# Pure imaginary
z4 <- 2i
stopifnot(Re(z4) == 0)
stopifnot(Im(z4) == 2)
cat("PASS: pure imaginary\n")

# is.complex
stopifnot(is.complex(1i))
stopifnot(!is.complex(1))
stopifnot(!is.complex("hello"))
cat("PASS: is.complex\n")

# as.complex
z5 <- as.complex(3)
stopifnot(is.complex(z5))
stopifnot(Re(z5) == 3)
stopifnot(Im(z5) == 0)
cat("PASS: as.complex\n")

# Re and Im on real numbers
stopifnot(Re(5) == 5)
stopifnot(Im(5) == 0)
cat("PASS: Re/Im on reals\n")

# Complex vector
z6 <- c(1+2i, 3+4i, 5+6i)
stopifnot(length(z6) == 3)
cat("PASS: complex vector length\n")

cat("\nAll basic complex tests passed!\n")
