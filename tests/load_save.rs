use r::Session;
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

// region: binary save() round-trip tests

#[test]
fn save_writes_binary_rdata_format_by_default() {
    let mut s = Session::new();
    let path = temp_path("binary-default");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- 42L
save(x, file = "{p}")
"#
    ))
    .unwrap();

    // Verify the file starts with gzip magic bytes (compressed RDX2).
    let bytes = fs::read(&path).unwrap();
    assert!(
        bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b,
        "expected gzip-compressed RDX2, first bytes: {:?}",
        &bytes[..bytes.len().min(4)]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn save_uncompressed_writes_rdx2_header() {
    let mut s = Session::new();
    let path = temp_path("binary-nocomp");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- 42L
save(x, file = "{p}", compress = FALSE)
"#
    ))
    .unwrap();

    // Verify the file starts with "RDX2\n".
    let bytes = fs::read(&path).unwrap();
    assert!(
        bytes.starts_with(b"RDX2\n"),
        "expected RDX2 header, first bytes: {:?}",
        &bytes[..bytes.len().min(10)]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_integer_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-int");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1L, 2L, NA_integer_, 4L)
save(x, file = "{p}")
x <- NULL
loaded <- load("{p}")
stopifnot(identical(loaded, "x"))
stopifnot(is.integer(x))
stopifnot(length(x) == 4)
stopifnot(x[1] == 1L)
stopifnot(x[2] == 2L)
stopifnot(is.na(x[3]))
stopifnot(x[4] == 4L)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_double_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-dbl");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1.5, NA_real_, -Inf, 0.0)
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.double(x))
stopifnot(x[1] == 1.5)
stopifnot(is.na(x[2]))
stopifnot(x[3] == -Inf)
stopifnot(x[4] == 0.0)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_character_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-chr");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c("hello", NA_character_, "world", "")
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.character(x))
stopifnot(x[1] == "hello")
stopifnot(is.na(x[2]))
stopifnot(x[3] == "world")
stopifnot(x[4] == "")
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_logical_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-lgl");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(TRUE, FALSE, NA)
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.logical(x))
stopifnot(x[1] == TRUE)
stopifnot(x[2] == FALSE)
stopifnot(is.na(x[3]))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_complex_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-cplx");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1+2i, 3-4i)
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.complex(x))
stopifnot(Re(x[1]) == 1)
stopifnot(Im(x[1]) == 2)
stopifnot(Re(x[2]) == 3)
stopifnot(Im(x[2]) == -4)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_named_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-save-named");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(a = 1L, b = 2L, c = 3L)
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.integer(x))
stopifnot(identical(names(x), c("a", "b", "c")))
stopifnot(x[["a"]] == 1L)
stopifnot(x[["c"]] == 3L)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_list() {
    let mut s = Session::new();
    let path = temp_path("rt-save-list");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- list(a = 1L, b = "hello", c = TRUE)
save(x, file = "{p}")
x <- NULL
load("{p}")
stopifnot(is.list(x))
stopifnot(x$a == 1L)
stopifnot(x$b == "hello")
stopifnot(x$c == TRUE)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_multiple_objects() {
    let mut s = Session::new();
    let path = temp_path("rt-save-multi");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- 42L
y <- "hello"
z <- c(TRUE, FALSE)
save(x, y, z, file = "{p}")
x <- NULL; y <- NULL; z <- NULL
loaded <- load("{p}")
stopifnot(identical(sort(loaded), c("x", "y", "z")))
stopifnot(x == 42L)
stopifnot(y == "hello")
stopifnot(identical(z, c(TRUE, FALSE)))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_data_frame() {
    let mut s = Session::new();
    let path = temp_path("rt-save-df");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
df <- data.frame(x = c(1L, 2L, 3L), y = c("a", "b", "c"))
save(df, file = "{p}")
df <- NULL
load("{p}")
stopifnot(is.data.frame(df))
stopifnot(identical(names(df), c("x", "y")))
stopifnot(nrow(df) == 3)
stopifnot(df$x[1] == 1L)
stopifnot(df$y[2] == "b")
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_binary_round_trip_null() {
    let mut s = Session::new();
    let path = temp_path("rt-save-null");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- NULL
save(x, file = "{p}")
x <- 99L
load("{p}")
stopifnot(is.null(x))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn save_ascii_true_falls_back_to_minirds() {
    let mut s = Session::new();
    let path = temp_path("ascii-save");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- 42L
save(x, file = "{p}", ascii = TRUE)
"#
    ))
    .unwrap();

    // The file should start with the miniRDS header, not RDX2.
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.starts_with("miniRDS1\n"),
        "expected miniRDS header, got: {:?}",
        &content[..content.len().min(20)]
    );

    // Round-trip via load should still work.
    s.eval_source(&format!(
        r#"
x <- NULL
load("{p}")
stopifnot(x == 42L)
"#
    ))
    .unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: save.image tests

#[test]
fn save_image_saves_all_global_env_bindings() {
    let path = temp_path("save-image");
    let p = quote_path(&path);

    let script = format!(
        r#"
x <- 42L
y <- "hello"
save.image(file = "{p}")
"#
    );

    let output = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &script])
        .output()
        .expect("failed to run miniR");

    assert!(
        output.status.success(),
        "save.image failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Load it back in a fresh session to confirm it works.
    let load_script = format!(
        r#"
loaded <- load("{p}")
stopifnot("x" %in% loaded)
stopifnot("y" %in% loaded)
stopifnot(x == 42L)
stopifnot(y == "hello")
"#
    );

    let output2 = Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &load_script])
        .output()
        .expect("failed to run miniR");

    assert!(
        output2.status.success(),
        "load after save.image failed: {}",
        String::from_utf8_lossy(&output2.stderr)
    );

    let _ = fs::remove_file(path);
}

// endregion

// region: saveRDS ascii parameter tests

#[test]
fn save_rds_ascii_true_writes_minirds() {
    let mut s = Session::new();
    let path = temp_path("saverds-ascii");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
saveRDS(42L, "{p}", ascii = TRUE)
"#
    ))
    .unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.starts_with("miniRDS1\n"),
        "expected miniRDS header with ascii=TRUE, got: {:?}",
        &content[..content.len().min(20)]
    );

    // Can still read it back.
    s.eval_source(&format!(
        r#"
y <- readRDS("{p}")
stopifnot(y == 42L)
"#
    ))
    .unwrap();

    let _ = fs::remove_file(path);
}

// endregion
