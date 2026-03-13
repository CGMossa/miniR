use std::process::Command;

#[test]
fn colnames_exposes_matrix_and_data_frame_labels() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
stopifnot(identical(colnames(m), c("x", "y")))

df <- data.frame(x = 1:2, y = 3:4)
stopifnot(identical(colnames(df), c("x", "y")))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
