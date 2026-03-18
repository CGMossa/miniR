//! Tests for package lifecycle hooks, system.file(), packageVersion(),
//! and getNamespace().

use r::Session;
use std::fs;

/// Create a minimal test package in a temp directory.
/// Returns the path to the library directory (parent of the package dir).
fn create_test_package(lib_dir: &std::path::Path, pkg_name: &str, version: &str) {
    let pkg_dir = lib_dir.join(pkg_name);
    let r_dir = pkg_dir.join("R");
    fs::create_dir_all(&r_dir).unwrap();

    // DESCRIPTION
    fs::write(
        pkg_dir.join("DESCRIPTION"),
        format!("Package: {pkg_name}\nVersion: {version}\nTitle: Test Package\nLicense: MIT\n"),
    )
    .unwrap();

    // NAMESPACE — export everything
    fs::write(pkg_dir.join("NAMESPACE"), "exportPattern(\"^[^.]\")\n").unwrap();
}

// region: system.file

#[test]
fn system_file_finds_description() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "testpkg", "1.2.3");

    let mut s = Session::new();
    // Set R_LIBS to our temp lib directory
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    let result = s
        .eval_source("system.file(\"DESCRIPTION\", package = \"testpkg\")")
        .unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert!(
        path.contains("testpkg"),
        "path should contain package name: {path}"
    );
    assert!(
        path.ends_with("DESCRIPTION"),
        "path should end with DESCRIPTION: {path}"
    );
    assert!(
        std::path::Path::new(&path).exists(),
        "returned path should exist: {path}"
    );
}

#[test]
fn system_file_returns_empty_for_missing_file() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "testpkg", "1.0.0");

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    let result = s
        .eval_source("system.file(\"nonexistent.txt\", package = \"testpkg\")")
        .unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(path, "", "missing file should return empty string");
}

#[test]
fn system_file_returns_empty_for_missing_package() {
    let mut s = Session::new();
    let result = s
        .eval_source("system.file(\"DESCRIPTION\", package = \"no_such_pkg_xyz\")")
        .unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(path, "", "missing package should return empty string");
}

#[test]
fn system_file_returns_pkg_dir_with_no_subpath() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "testpkg", "1.0.0");

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    let result = s.eval_source("system.file(package = \"testpkg\")").unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert!(
        path.contains("testpkg"),
        "should return package directory: {path}"
    );
    assert!(
        std::path::Path::new(&path).is_dir(),
        "returned path should be a directory: {path}"
    );
}

#[test]
fn system_file_no_package_arg_returns_empty() {
    let mut s = Session::new();
    let result = s.eval_source("system.file(\"DESCRIPTION\")").unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(path, "", "no package arg should return empty string");
}

// endregion

// region: system.file with loaded namespace

#[test]
fn system_file_finds_file_in_loaded_package() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "mypkg", "2.0.0");

    // Also create a data file to search for
    let data_dir = lib_dir.join("mypkg").join("data");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(data_dir.join("test.csv"), "a,b\n1,2\n").unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // Load the package first
    s.eval_source("library(\"mypkg\")").unwrap();

    // Now system.file should find the data file
    let result = s
        .eval_source("system.file(\"data/test.csv\", package = \"mypkg\")")
        .unwrap();
    let path = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert!(
        path.ends_with("data/test.csv") || path.ends_with("data\\test.csv"),
        "should find data file: {path}"
    );
}

// endregion

// region: packageVersion

#[test]
fn package_version_returns_version_string() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "verpkg", "3.14.159");

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    let result = s.eval_source("packageVersion(\"verpkg\")").unwrap();
    let version = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(version, "3.14.159");
}

#[test]
fn package_version_from_loaded_namespace() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "loadedpkg", "0.9.1");

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // Load the package first
    s.eval_source("library(\"loadedpkg\")").unwrap();

    let result = s.eval_source("packageVersion(\"loadedpkg\")").unwrap();
    let version = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(version, "0.9.1");
}

#[test]
fn package_version_errors_for_missing_package() {
    let mut s = Session::new();
    let result = s.eval_source("packageVersion(\"no_such_pkg_xyz\")");
    assert!(result.is_err(), "should error for missing package");
}

// endregion

// region: getNamespace

#[test]
fn get_namespace_returns_environment_for_loaded_package() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "nspkg", "1.0.0");

    // Write an R file that defines a function
    fs::write(
        lib_dir.join("nspkg").join("R").join("funcs.R"),
        "myfun <- function() 42\n",
    )
    .unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // Load the namespace
    s.eval_source("loadNamespace(\"nspkg\")").unwrap();

    // getNamespace should return the namespace env
    s.eval_source(
        r#"
        ns <- getNamespace("nspkg")
        stopifnot(is.environment(ns))
    "#,
    )
    .unwrap();
}

#[test]
fn get_namespace_auto_loads_unloaded_package() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "autopkg", "1.0.0");

    fs::write(
        lib_dir.join("autopkg").join("R").join("funcs.R"),
        "hello <- function() \"world\"\n",
    )
    .unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // getNamespace should auto-load the namespace
    s.eval_source(
        r#"
        ns <- getNamespace("autopkg")
        stopifnot(is.environment(ns))
        stopifnot(isNamespaceLoaded("autopkg"))
    "#,
    )
    .unwrap();
}

#[test]
fn get_namespace_falls_back_to_base_for_builtin() {
    let mut s = Session::new();
    // "base" namespace should always be accessible
    s.eval_source(
        r#"
        ns <- getNamespace("base")
        stopifnot(is.environment(ns))
    "#,
    )
    .unwrap();
}

// endregion

// region: .onLoad and .onAttach hooks

#[test]
fn on_load_hook_is_called() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "hookpkg", "1.0.0");

    // .onLoad writes a marker file so we can verify it ran
    let marker = lib_dir.join("hookpkg").join("onload_marker.txt");
    fs::write(
        lib_dir.join("hookpkg").join("R").join("zzz.R"),
        format!(
            r#"
.onLoad <- function(libname, pkgname) {{
    writeLines(paste(libname, pkgname), "{}")
}}
"#,
            marker.display()
        ),
    )
    .unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library(\"hookpkg\")").unwrap();

    // Verify the .onLoad hook ran by checking the marker file
    assert!(marker.exists(), ".onLoad should have created marker file");
    let contents = fs::read_to_string(&marker).unwrap();
    assert!(
        contents.contains("hookpkg"),
        "marker should contain package name: {contents}"
    );
}

#[test]
fn on_attach_hook_is_called() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "attachpkg", "1.0.0");

    // .onAttach writes a marker file so we can verify it ran
    let marker = lib_dir.join("attachpkg").join("onattach_marker.txt");
    fs::write(
        lib_dir.join("attachpkg").join("R").join("zzz.R"),
        format!(
            r#"
.onAttach <- function(libname, pkgname) {{
    writeLines(paste(libname, pkgname), "{}")
}}
"#,
            marker.display()
        ),
    )
    .unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library(\"attachpkg\")").unwrap();

    // Verify the .onAttach hook ran by checking the marker file
    assert!(marker.exists(), ".onAttach should have created marker file");
    let contents = fs::read_to_string(&marker).unwrap();
    assert!(
        contents.contains("attachpkg"),
        "marker should contain package name: {contents}"
    );
}

#[test]
fn on_load_runs_before_on_attach() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "orderpkg", "1.0.0");

    // Both hooks append to a shared marker file so we can verify order
    let marker = lib_dir.join("orderpkg").join("order_marker.txt");
    fs::write(
        lib_dir.join("orderpkg").join("R").join("zzz.R"),
        format!(
            r#"
.onLoad <- function(libname, pkgname) {{
    writeLines("onLoad", "{marker}")
}}

.onAttach <- function(libname, pkgname) {{
    prev <- readLines("{marker}")
    writeLines(c(prev, "onAttach"), "{marker}")
}}
"#,
            marker = marker.display()
        ),
    )
    .unwrap();

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library(\"orderpkg\")").unwrap();

    // Verify both hooks ran in the correct order
    assert!(marker.exists(), "marker file should exist");
    let contents = fs::read_to_string(&marker).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "should have two lines: {:?}", lines);
    assert_eq!(lines[0], "onLoad", "first hook should be .onLoad");
    assert_eq!(lines[1], "onAttach", "second hook should be .onAttach");
}

// endregion

// region: isNamespaceLoaded

#[test]
fn is_namespace_loaded_reports_correctly() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();
    create_test_package(&lib_dir, "checkpkg", "1.0.0");

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // Not loaded yet
    s.eval_source("stopifnot(!isNamespaceLoaded(\"checkpkg\"))")
        .unwrap();

    // Load it
    s.eval_source("library(\"checkpkg\")").unwrap();

    // Now it should be loaded
    s.eval_source("stopifnot(isNamespaceLoaded(\"checkpkg\"))")
        .unwrap();
}

// endregion
