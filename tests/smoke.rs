use std::process::Command;

/// End-to-end smoke test covering ops, assignment, indexing, and datetime.
#[test]
fn smoke_test_ops_assignment_indexing_datetime() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"
# Arithmetic (ops.rs)
stopifnot(1 + 2 == 3)
stopifnot(3 * 4 == 12)
stopifnot(10 %% 3 == 1)
stopifnot(2^10 == 1024)
stopifnot(7 %/% 2 == 3)

# Comparison (ops.rs)
stopifnot(1 < 2)
stopifnot("a" == "a")
stopifnot(!(3 > 5))

# Range and %in% (ops.rs)
stopifnot(identical(1:5, c(1L, 2L, 3L, 4L, 5L)))
stopifnot(3 %in% 1:5)
stopifnot(!(6 %in% 1:5))

# Logical (ops.rs)
stopifnot(identical(c(TRUE, FALSE) & c(TRUE, TRUE), c(TRUE, FALSE)))
stopifnot(identical(c(TRUE, FALSE) | c(FALSE, FALSE), c(TRUE, FALSE)))

# Unary (ops.rs)
stopifnot(-5 == -5)
stopifnot(!FALSE)

# Assignment (assignment.rs)
x <- c(10, 20, 30)
x[2] <- 99
stopifnot(x[2] == 99)

lst <- list(a = 1, b = 2)
lst$c <- 3
stopifnot(lst$c == 3)
lst[["d"]] <- 4
stopifnot(lst[["d"]] == 4)

# Super-assignment
f <- function() { y <<- 42 }
f()
stopifnot(y == 42)

# Replacement function
names(lst) <- c("w", "x", "y", "z")
stopifnot(identical(names(lst), c("w", "x", "y", "z")))

# Indexing read (indexing.rs)
v <- c(10, 20, 30, 40, 50)
stopifnot(identical(v[c(1, 3)], c(10, 30)))
stopifnot(identical(v[-2], c(10, 30, 40, 50)))
stopifnot(identical(v[c(TRUE, FALSE, TRUE, FALSE, TRUE)], c(10, 30, 50)))
stopifnot(v[[3]] == 30)

# Dollar/list indexing
info <- list(name = "test", value = 123)
stopifnot(info$name == "test")
stopifnot(info[["value"]] == 123)

# Datetime (datetime.rs)
d <- as.Date("2024-06-15")
stopifnot(weekdays(d) == "Saturday")
stopifnot(months(d) == "June")
stopifnot(quarters(d) == "Q2")
stopifnot(format.Date(d) == "2024-06-15")
stopifnot(format.Date(d, format = "%Y/%m/%d") == "2024/06/15")

# POSIXct
ct <- as.POSIXct("2024-01-15 10:30:00", tz = "UTC")
stopifnot(format.POSIXct(ct, format = "%H:%M:%S") == "10:30:00")

# POSIXlt components
lt <- as.POSIXlt("2024-06-15 14:30:45", tz = "UTC")
stopifnot(lt$hour == 14)
stopifnot(lt$min == 30)
stopifnot(lt$sec == 45)

# S3 dispatch for print/format
stopifnot(format(as.Date("2024-12-25")) == "2024-12-25")

cat("smoke test passed\n")
"#,
        ])
        .output()
        .expect("failed to run miniR");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "smoke test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("smoke test passed"),
        "missing completion marker:\nstdout: {stdout}\nstderr: {stderr}"
    );
}
