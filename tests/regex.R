# Test regex support

# grepl with regex
stopifnot(grepl("^hello", "hello world"))
stopifnot(!grepl("^world", "hello world"))
stopifnot(grepl("[0-9]+", "abc123"))
stopifnot(!grepl("[0-9]+", "abcdef"))

# grepl with fixed
stopifnot(grepl(".", "hello.world", fixed = TRUE))
stopifnot(!grepl(".", "helloXworld", fixed = TRUE))

# grepl with ignore.case
stopifnot(grepl("HELLO", "hello world", ignore.case = TRUE))

# grep
x <- c("apple", "banana", "cherry", "date")
stopifnot(identical(grep("an", x), 2L))
stopifnot(identical(grep("a", x), c(1L, 2L, 4L)))
stopifnot(identical(grep("a", x, value = TRUE), c("apple", "banana", "date")))

# sub - replace first match only
stopifnot(sub("o", "0", "foo bar boo") == "f0o bar boo")

# gsub - replace all matches
stopifnot(gsub("o", "0", "foo bar boo") == "f00 bar b00")

# gsub with regex
stopifnot(gsub("[aeiou]", "*", "hello") == "h*ll*")

# sub with backreferences
stopifnot(sub("(\\w+) (\\w+)", "\\2 \\1", "hello world") == "world hello")

# regexpr
m <- regexpr("[0-9]+", "abc123def")
stopifnot(m == 4L)
stopifnot(attr(m, "match.length") == 3L)

# regexpr no match
m2 <- regexpr("[0-9]+", "abcdef")
stopifnot(m2 == -1L)

# gregexpr — result has match.length attribute, so compare values not identical
gm <- gregexpr("[0-9]+", "abc123def456")
stopifnot(all(gm[[1]] == c(4L, 10L)))
stopifnot(all(attr(gm[[1]], "match.length") == c(3L, 3L)))

# regmatches with regexpr
x <- "The year is 2026"
m <- regexpr("[0-9]+", x)
stopifnot(regmatches(x, m) == "2026")

# regmatches with gregexpr
x <- "abc123def456"
gm <- gregexpr("[0-9]+", x)
stopifnot(identical(regmatches(x, gm)[[1]], c("123", "456")))

cat("All regex tests passed!\n")
