use std::process::Command;

#[test]
fn data_frame_recycles_and_honors_row_names() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"recycled <- data.frame(x = c("A", "B"), y = "C")
stopifnot(
  nrow(recycled) == 2L,
  identical(recycled$y, c("C", "C")),
  identical(row.names(recycled), c("1", "2"))
)

named_rows <- data.frame(x = c(a = 1, b = 2))
stopifnot(
  identical(row.names(named_rows), c("a", "b")),
  is.null(names(named_rows$x))
)

auto_rows <- data.frame(x = c(a = 1, b = 2), row.names = NULL)
stopifnot(identical(row.names(auto_rows), c("1", "2")))

empty <- data.frame(row.names = 1:4)
stopifnot(
  nrow(empty) == 4L,
  length(names(empty)) == 0L,
  identical(row.names(empty), c("1", "2", "3", "4"))
)

from_matrix <- data.frame(matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y"))))
stopifnot(
  identical(names(from_matrix), c("x", "y")),
  identical(from_matrix$y, c(3L, 4L)),
  identical(row.names(from_matrix), c("r1", "r2"))
)

from_list <- data.frame(list(a = 1:2, b = 3:4))
stopifnot(identical(names(from_list), c("a", "b")))

factored <- data.frame(x = c("b", "a"), stringsAsFactors = TRUE)
stopifnot(is.factor(factored$x))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn data_frame_rejects_incompatible_row_counts() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", "data.frame(x = 1:3, y = 1:2)"])
        .output()
        .expect("failed to run miniR");

    assert!(!output.status.success(), "command unexpectedly succeeded");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("arguments imply differing number of rows"),
        "unexpected stderr: {stderr}"
    );
}
