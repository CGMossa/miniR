use std::process::Command;

#[test]
fn as_vector_strips_vector_attributes() {
    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            r#"x <- structure(1:3, names = c("a", "b", "c"), class = "foo");
y <- as.vector(x);
print(is.null(attributes(y)))"#,
        ])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1] TRUE"), "unexpected stdout: {stdout}");
}
