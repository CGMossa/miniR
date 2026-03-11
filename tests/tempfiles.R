# Test tempfile() and tempdir()

# tempdir() returns a unique session directory (not system temp)
td <- tempdir()
stopifnot(is.character(td))
stopifnot(nchar(td) > 0)
cat("PASS: tempdir() returns a path\n")

# tempdir() returns a directory that exists
stopifnot(dir.exists(td))
cat("PASS: tempdir() directory exists\n")

# tempfile() returns paths inside the session temp dir by default
tf1 <- tempfile()
stopifnot(startsWith(tf1, td))
cat("PASS: tempfile() path is inside tempdir()\n")

# tempfile() returns unique paths
tf2 <- tempfile()
stopifnot(tf1 != tf2)
cat("PASS: tempfile() returns unique paths\n")

# tempfile() with pattern
tf3 <- tempfile(pattern = "mydata")
stopifnot(grepl("mydata", tf3))
cat("PASS: tempfile() with pattern\n")

# tempfile() with fileext
tf4 <- tempfile(fileext = ".csv")
stopifnot(grepl("\\.csv$", tf4))
cat("PASS: tempfile() with fileext\n")

# tempfile() with pattern and fileext
tf5 <- tempfile(pattern = "report", fileext = ".txt")
stopifnot(grepl("report", tf5))
stopifnot(grepl("\\.txt$", tf5))
cat("PASS: tempfile() with pattern and fileext\n")

# Can write to tempfile path
tf6 <- tempfile(fileext = ".txt")
writeLines("hello temp", tf6)
stopifnot(file.exists(tf6))
content <- readLines(tf6)
stopifnot(content == "hello temp")
cat("PASS: can write to and read from tempfile\n")

cat("\nAll tempfile tests passed!\n")
