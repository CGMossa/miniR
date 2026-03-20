use r::Session;

// region: Sys.getenv

#[test]
fn sys_getenv_returns_value_for_set_var() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(TEST_GETENV_VAR = "hello123")
val <- Sys.getenv("TEST_GETENV_VAR")
stopifnot(val == "hello123")
"#,
    )
    .unwrap();
}

#[test]
fn sys_getenv_returns_empty_string_for_unset_var_by_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
val <- Sys.getenv("NONEXISTENT_VAR_XYZ_123")
stopifnot(val == "")
"#,
    )
    .unwrap();
}

#[test]
fn sys_getenv_unset_parameter_custom_default() {
    let mut s = Session::new();
    s.eval_source(
        r#"
val <- Sys.getenv("NONEXISTENT_VAR_XYZ_456", unset = "fallback")
stopifnot(val == "fallback")
"#,
    )
    .unwrap();
}

#[test]
fn sys_getenv_unset_na_returns_na() {
    let mut s = Session::new();
    s.eval_source(
        r#"
val <- Sys.getenv("NONEXISTENT_VAR_XYZ_789", unset = NA)
stopifnot(is.na(val))
"#,
    )
    .unwrap();
}

#[test]
fn sys_getenv_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(TESTVAR_A = "aaa", TESTVAR_B = "bbb")
vals <- Sys.getenv(c("TESTVAR_A", "TESTVAR_B"))
stopifnot(length(vals) == 2)
stopifnot(vals[1] == "aaa")
stopifnot(vals[2] == "bbb")
"#,
    )
    .unwrap();
}

#[test]
fn sys_getenv_no_args_returns_all_vars() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(UNIQUE_MARKER_VAR = "found_it")
all_vars <- Sys.getenv()
# Should be a character vector with length > 0
stopifnot(is.character(all_vars))
stopifnot(length(all_vars) > 0)
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.setenv

#[test]
fn sys_setenv_sets_named_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Sys.setenv(FOO = "bar", BAZ = "qux")
stopifnot(identical(result, TRUE))
stopifnot(Sys.getenv("FOO") == "bar")
stopifnot(Sys.getenv("BAZ") == "qux")
"#,
    )
    .unwrap();
}

#[test]
fn sys_setenv_overwrites_existing() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(OVERWRITE_TEST = "first")
stopifnot(Sys.getenv("OVERWRITE_TEST") == "first")
Sys.setenv(OVERWRITE_TEST = "second")
stopifnot(Sys.getenv("OVERWRITE_TEST") == "second")
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.unsetenv

#[test]
fn sys_unsetenv_removes_variable() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(UNSET_TEST_VAR = "exists")
stopifnot(Sys.getenv("UNSET_TEST_VAR") == "exists")

result <- Sys.unsetenv("UNSET_TEST_VAR")
stopifnot(identical(result, TRUE))
stopifnot(Sys.getenv("UNSET_TEST_VAR") == "")
"#,
    )
    .unwrap();
}

#[test]
fn sys_unsetenv_vectorized() {
    let mut s = Session::new();
    s.eval_source(
        r#"
Sys.setenv(UNSET_A = "a", UNSET_B = "b")
result <- Sys.unsetenv(c("UNSET_A", "UNSET_B"))
stopifnot(length(result) == 2)
stopifnot(all(result))
stopifnot(Sys.getenv("UNSET_A") == "")
stopifnot(Sys.getenv("UNSET_B") == "")
"#,
    )
    .unwrap();
}

#[test]
fn sys_unsetenv_per_interpreter_isolation() {
    // Two sessions should have independent env var state
    let mut s1 = Session::new();
    let mut s2 = Session::new();

    s1.eval_source(r#"Sys.setenv(ISOLATION_VAR = "session1")"#)
        .unwrap();
    s2.eval_source(r#"Sys.setenv(ISOLATION_VAR = "session2")"#)
        .unwrap();

    // Unsetting in s1 should not affect s2
    s1.eval_source(r#"Sys.unsetenv("ISOLATION_VAR")"#).unwrap();
    s1.eval_source(r#"stopifnot(Sys.getenv("ISOLATION_VAR") == "")"#)
        .unwrap();
    s2.eval_source(r#"stopifnot(Sys.getenv("ISOLATION_VAR") == "session2")"#)
        .unwrap();
}

// endregion

// region: system with intern

#[test]
fn system_intern_true_captures_stdout() {
    let mut s = Session::new();
    s.eval_source(
        r#"
output <- system("echo hello_world", intern = TRUE)
stopifnot(is.character(output))
stopifnot(length(output) >= 1)
stopifnot(output[1] == "hello_world")
"#,
    )
    .unwrap();
}

#[test]
fn system_intern_true_multiline() {
    let mut s = Session::new();
    s.eval_source(
        r#"
output <- system("printf 'line1\nline2\nline3'", intern = TRUE)
stopifnot(length(output) == 3)
stopifnot(output[1] == "line1")
stopifnot(output[2] == "line2")
stopifnot(output[3] == "line3")
"#,
    )
    .unwrap();
}

#[test]
fn system_intern_false_returns_exit_code() {
    let mut s = Session::new();
    s.eval_source(
        r#"
code <- system("true")
stopifnot(is.integer(code))
stopifnot(code == 0L)

code2 <- system("false")
stopifnot(code2 != 0L)
"#,
    )
    .unwrap();
}

// endregion

// region: system2 with stdout/stderr capture

#[test]
fn system2_captures_stdout() {
    let mut s = Session::new();
    s.eval_source(
        r#"
output <- system2("echo", args = c("captured_output"), stdout = TRUE)
stopifnot(is.character(output))
stopifnot(length(output) >= 1)
stopifnot(output[1] == "captured_output")
"#,
    )
    .unwrap();
}

#[test]
fn system2_no_capture_returns_exit_code() {
    let mut s = Session::new();
    s.eval_source(
        r#"
code <- system2("true")
stopifnot(is.integer(code))
stopifnot(code == 0L)
"#,
    )
    .unwrap();
}

#[test]
fn system2_captures_stderr() {
    let mut s = Session::new();
    s.eval_source(
        r#"
output <- system2("sh", args = c("-c", "echo err_msg >&2"), stderr = TRUE)
stopifnot(is.character(output))
stopifnot(length(output) >= 1)
stopifnot(output[1] == "err_msg")
"#,
    )
    .unwrap();
}

#[test]
fn system2_status_attribute_on_capture() {
    let mut s = Session::new();
    s.eval_source(
        r#"
output <- system2("sh", args = c("-c", "echo ok; exit 42"), stdout = TRUE)
stopifnot(output[1] == "ok")
status <- attr(output, "status")
stopifnot(!is.null(status))
stopifnot(status == 42L)
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.which

#[test]
fn sys_which_finds_common_programs() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# sh should be findable on any Unix system
result <- Sys.which("sh")
stopifnot(is.character(result))
stopifnot(nchar(result[1]) > 0)
# Should be an absolute path
stopifnot(startsWith(result[1], "/"))
"#,
    )
    .unwrap();
}

#[test]
fn sys_which_returns_empty_for_missing() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Sys.which("nonexistent_program_xyz_123")
stopifnot(is.character(result))
stopifnot(result[1] == "")
"#,
    )
    .unwrap();
}

#[test]
fn sys_which_vectorized_input() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Sys.which(c("sh", "nonexistent_xyz"))
stopifnot(length(result) == 2)
stopifnot(nchar(result[1]) > 0)   # sh found
stopifnot(result[2] == "")         # nonexistent not found
"#,
    )
    .unwrap();
}

#[test]
fn sys_which_returns_named_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- Sys.which(c("sh", "ls"))
nms <- names(result)
stopifnot(!is.null(nms))
stopifnot(nms[1] == "sh")
stopifnot(nms[2] == "ls")
"#,
    )
    .unwrap();
}

// endregion

// region: normalizePath

#[test]
fn normalize_path_resolves_existing() {
    let mut s = Session::new();
    s.eval_source(
        r#"
# /tmp should exist on any Unix system
result <- normalizePath("/tmp")
stopifnot(is.character(result))
stopifnot(nchar(result) > 0)
"#,
    )
    .unwrap();
}

#[test]
fn normalize_path_must_work_true_errors_on_missing() {
    let mut s = Session::new();
    let result = s.eval_source(r#"normalizePath("/nonexistent/path/xyz_test", mustWork = TRUE)"#);
    assert!(
        result.is_err(),
        "Expected error for missing path with mustWork=TRUE"
    );
}

#[test]
fn normalize_path_must_work_false_returns_original() {
    let mut s = Session::new();
    s.eval_source(
        r#"
result <- normalizePath("/nonexistent/path/xyz_test", mustWork = FALSE)
stopifnot(result == "/nonexistent/path/xyz_test")
"#,
    )
    .unwrap();
}

// endregion

// region: R.home

#[test]
fn r_home_returns_string() {
    let mut s = Session::new();
    s.eval_source(
        r#"
home <- R.home()
stopifnot(is.character(home))
stopifnot(nchar(home) > 0)
"#,
    )
    .unwrap();
}

#[test]
fn r_home_component_appended() {
    let mut s = Session::new();
    s.eval_source(
        r#"
home <- R.home()
bin <- R.home("bin")
stopifnot(endsWith(bin, "/bin"))
stopifnot(startsWith(bin, home))

lib <- R.home("lib")
stopifnot(endsWith(lib, "/lib"))

etc <- R.home("etc")
stopifnot(endsWith(etc, "/etc"))
"#,
    )
    .unwrap();
}

// endregion

// region: Sys.info

#[test]
fn sys_info_returns_named_character_vector() {
    let mut s = Session::new();
    s.eval_source(
        r#"
info <- Sys.info()
stopifnot(is.character(info))
stopifnot(length(info) == 7)

nms <- names(info)
stopifnot(!is.null(nms))
stopifnot("sysname" %in% nms)
stopifnot("nodename" %in% nms)
stopifnot("release" %in% nms)
stopifnot("version" %in% nms)
stopifnot("machine" %in% nms)
stopifnot("login" %in% nms)
stopifnot("user" %in% nms)
"#,
    )
    .unwrap();
}

#[test]
fn sys_info_sysname_is_populated() {
    let mut s = Session::new();
    s.eval_source(
        r#"
info <- Sys.info()
sysname <- info[1]
stopifnot(nchar(sysname) > 0)
# On macOS it should be "Darwin", on Linux "Linux"
stopifnot(sysname %in% c("Darwin", "Linux", "Windows"))
"#,
    )
    .unwrap();
}

#[test]
fn sys_info_all_fields_non_empty() {
    let mut s = Session::new();
    s.eval_source(
        r#"
info <- Sys.info()
for (i in seq_along(info)) {
    stopifnot(nchar(info[i]) > 0)
}
"#,
    )
    .unwrap();
}

// endregion

// region: shell.exec (verify it exists as a builtin, don't actually open anything)

#[test]
fn shell_exec_rejects_missing_arg() {
    let mut s = Session::new();
    let result = s.eval_source(r#"shell.exec()"#);
    assert!(result.is_err(), "shell.exec() with no args should error");
}

// endregion
