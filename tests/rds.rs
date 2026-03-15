use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("minir-{name}-{suffix}.rds"))
}

fn quote_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn read_rds_and_save_rds_round_trip_common_values() {
    let vector_path = temp_path("vector");
    let matrix_path = temp_path("matrix");
    let data_frame_path = temp_path("dataframe");
    let factor_path = temp_path("factor");

    let script = format!(
        r#"
x <- structure(c(1L, NA_integer_, 3L), names = c("a", "b", "c"))
saveRDS(x, "{vector_path}")
x2 <- readRDS("{vector_path}")
stopifnot(identical(x2, x))

m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("x", "y")))
saveRDS(m, "{matrix_path}")
m2 <- readRDS("{matrix_path}")
stopifnot(
  inherits(m2, "matrix"),
  identical(dim(m2), dim(m)),
  identical(rownames(m2), rownames(m)),
  identical(colnames(m2), colnames(m)),
  identical(as.vector(m2), as.vector(m))
)

df <- data.frame(x = c(1L, 2L), y = c("u", "v"), row.names = c("r1", "r2"))
saveRDS(df, "{data_frame_path}")
df2 <- readRDS("{data_frame_path}")
stopifnot(
  identical(names(df2), names(df)),
  identical(row.names(df2), row.names(df)),
  identical(df2$x, df$x),
  identical(df2$y, df$y)
)

f <- factor(c("b", "a"))
saveRDS(f, "{factor_path}")
f2 <- readRDS("{factor_path}")
stopifnot(
  is.factor(f2),
  identical(levels(f2), levels(f)),
  identical(as.integer(f2), as.integer(f))
)
"#,
        vector_path = quote_path(&vector_path),
        matrix_path = quote_path(&matrix_path),
        data_frame_path = quote_path(&data_frame_path),
        factor_path = quote_path(&factor_path),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &script])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for path in [vector_path, matrix_path, data_frame_path, factor_path] {
        let _ = fs::remove_file(path);
    }
}

#[test]
fn read_rds_rejects_non_minirds_files() {
    let path = temp_path("invalid");
    fs::write(&path, "not a miniRDS file\n").expect("failed to seed invalid file");

    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &format!("readRDS(\"{}\")", quote_path(&path))])
        .output()
        .expect("failed to run miniR");

    assert!(!output.status.success(), "command unexpectedly succeeded");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("miniRDS"), "unexpected stderr: {stderr}");

    let _ = fs::remove_file(path);
}
