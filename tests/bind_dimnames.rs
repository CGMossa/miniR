use std::process::Command;

#[test]
fn bind_preserves_matrix_dimnames_in_common_cases() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"m1 <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
m2 <- matrix(5:8, nrow = 2, dimnames = list(c("r1", "r2"), c("u", "v")))

cb <- cbind(m1, m2)
stopifnot(
  identical(rownames(cb), c("r1", "r2")),
  identical(colnames(cb), c("x", "y", "u", "v"))
)

rb <- rbind(m1, m2)
stopifnot(
  identical(rownames(rb), c("r1", "r2", "r1", "r2")),
  identical(colnames(rb), c("x", "y"))
)

unnamed <- matrix(1:4, nrow = 2)
named_rows <- matrix(5:8, nrow = 2, dimnames = list(c("a", "b"), c("u", "v")))
stopifnot(identical(rownames(cbind(unnamed, named_rows)), c("a", "b")))
stopifnot(identical(colnames(rbind(unnamed, named_rows)), c("u", "v")))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
