use std::process::Command;

#[test]
fn unname_strips_matrix_dimnames() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
um <- unname(m)
stopifnot(is.null(rownames(um)), is.null(colnames(um)))

df <- data.frame(x = 1:2, y = 3:4)
udf <- unname(df)
stopifnot(
  is.null(names(udf)),
  identical(row.names(udf), c("1", "2")),
  is.null(colnames(udf))
)"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
