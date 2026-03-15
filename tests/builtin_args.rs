use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use r::Session;

fn temp_path(name: &str, extension: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("minir-{name}-{suffix}.{extension}"))
}

fn quote_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn interpreter_builtins_honor_named_environment_arguments() {
    let mut session = Session::new();
    let value = session
        .eval_source(
            r#"
e <- new.env()
assign("x", 41L, envir = e)

stopifnot(exists("x", envir = e))
stopifnot(!exists("x"))
stopifnot(identical(get("x", envir = e), 41L))

eval(quote(y <- x + 1L), envir = e)
ls(envir = e)
"#,
        )
        .expect("failed to evaluate environment builtin script")
        .value;

    assert_eq!(
        value.as_vector().map(|vector| vector.to_characters()),
        Some(vec![Some("x".to_string()), Some("y".to_string())]),
    );
}

#[test]
fn io_builtins_accept_named_argument_forms() {
    let text_path = temp_path("builtin-args-lines", "txt");
    let rds_path = temp_path("builtin-args-rds", "rds");
    let mut session = Session::new();
    let script = format!(
        r#"
path <- file.path("alpha", "beta", fsep = ":")
writeLines(c("one", "two"), con = "{text_path}", sep = "|")
saveRDS(object = path, file = "{rds_path}")

c(
  path,
  readLines(con = "{text_path}", n = 1L),
  readRDS(file = "{rds_path}")
)
"#,
        text_path = quote_path(&text_path),
        rds_path = quote_path(&rds_path),
    );

    let value = session
        .eval_source(&script)
        .expect("failed to evaluate IO builtin script")
        .value;

    assert_eq!(
        value.as_vector().map(|vector| vector.to_characters()),
        Some(vec![
            Some("alpha:beta".to_string()),
            Some("one|two|".to_string()),
            Some("alpha:beta".to_string()),
        ]),
    );

    let _ = fs::remove_file(text_path);
    let _ = fs::remove_file(rds_path);
}
