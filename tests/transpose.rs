use std::process::Command;

#[test]
fn transpose_swaps_matrix_dimnames() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"m <- matrix(
  1:6,
  nrow = 2,
  dimnames = list(c("r1", "r2"), c("x", "y", "z"))
)
tm <- t(m)
stopifnot(
  identical(dim(tm), c(3L, 2L)),
  identical(rownames(tm), c("x", "y", "z")),
  identical(colnames(tm), c("r1", "r2"))
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
