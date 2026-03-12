use std::process::Command;

#[test]
fn dim_reports_data_frame_shape() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"df <- data.frame(x = 1:2, y = 3:4)
stopifnot(identical(dim(df), c(2L, 2L)))

empty <- data.frame(row.names = 1:4)
stopifnot(identical(dim(empty), c(4L, 0L)))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
