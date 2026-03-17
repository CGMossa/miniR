use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("minir-{name}-{suffix}.RData"))
}

fn quote_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn save_and_load_round_trip_current_environment_bindings() {
    let path = temp_path("workspace-global");

    let script = format!(
        r#"
x <- structure(c(1L, 2L), names = c("a", "b"))
y <- data.frame(flag = c(TRUE, FALSE), row.names = c("r1", "r2"))
expected_x <- x
expected_y <- y

save(x, y, file = "{path}")

x <- 0L
y <- NULL

loaded <- load("{path}")

stopifnot(
  identical(loaded, c("x", "y")),
  identical(x, expected_x),
  identical(names(y), names(expected_y)),
  identical(row.names(y), row.names(expected_y)),
  identical(y$flag, expected_y$flag)
)
"#,
        path = quote_path(&path),
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

    let _ = fs::remove_file(path);
}

#[test]
fn save_and_load_support_list_and_explicit_environments() {
    let path = temp_path("workspace-env");

    let script = format!(
        r#"
source_env <- new.env()
evalq({{ number <- 42L; label <- "ok" }}, source_env)

save(list = c("number", "label"), file = "{path}", envir = source_env)

target_env <- new.env()
loaded <- load("{path}", envir = target_env)

stopifnot(
  identical(loaded, c("number", "label")),
  identical(ls(target_env), c("label", "number")),
  identical(evalq(number, target_env), 42L),
  identical(evalq(label, target_env), "ok")
)
"#,
        path = quote_path(&path),
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

    let _ = fs::remove_file(path);
}

#[test]
fn load_rejects_non_workspace_minirds_files() {
    let path = temp_path("workspace-invalid");

    let seed_output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args([
            "-e",
            &format!("saveRDS(list(x = 1L), \"{}\")", quote_path(&path)),
        ])
        .output()
        .expect("failed to seed miniRDS file");

    assert!(
        seed_output.status.success(),
        "seed failed: {}",
        String::from_utf8_lossy(&seed_output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &format!("load(\"{}\")", quote_path(&path))])
        .output()
        .expect("failed to run miniR");

    assert!(!output.status.success(), "command unexpectedly succeeded");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a recognized workspace file"),
        "unexpected stderr: {stderr}"
    );

    let _ = fs::remove_file(path);
}
