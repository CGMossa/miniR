//! Tests for S3 method registration via NAMESPACE S3method() directives.

use r::Session;
use std::fs;

/// Create a test package with an S3method() NAMESPACE directive.
fn create_s3_package(
    lib_dir: &std::path::Path,
    pkg_name: &str,
    namespace_content: &str,
    r_code: &str,
) {
    let pkg_dir = lib_dir.join(pkg_name);
    let r_dir = pkg_dir.join("R");
    fs::create_dir_all(&r_dir).unwrap();

    fs::write(
        pkg_dir.join("DESCRIPTION"),
        format!("Package: {pkg_name}\nVersion: 1.0.0\nTitle: Test S3 Package\nLicense: MIT\n"),
    )
    .unwrap();

    fs::write(pkg_dir.join("NAMESPACE"), namespace_content).unwrap();
    fs::write(r_dir.join("methods.R"), r_code).unwrap();
}

// region: S3method registration

#[test]
fn s3method_print_dispatches_to_registered_method() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();

    // Package with S3method(print, foo) and a print.foo method
    create_s3_package(
        &lib_dir,
        "foopkg",
        "S3method(print, foo)\nexport(make_foo)\n",
        r#"
make_foo <- function() {
    x <- list(value = 42)
    class(x) <- "foo"
    x
}

print.foo <- function(x, ...) {
    cat("foo:", x$value, "\n")
}
"#,
    );

    let mut s = Session::new_with_captured_output();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    // Load the package
    s.eval_source("library('foopkg')").unwrap();

    // Create a foo object and print it — should dispatch to print.foo
    s.eval_source("obj <- make_foo()").unwrap();
    s.eval_source("print(obj)").unwrap();

    let output = s.captured_stdout();
    assert!(
        output.contains("foo: 42"),
        "print.foo should have been dispatched via S3 registry, got: {output}"
    );
}

#[test]
fn s3method_with_explicit_method_name() {
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();

    // Package where the method function has a non-standard name
    create_s3_package(
        &lib_dir,
        "barpkg",
        "S3method(format, bar, my_format_bar)\nexport(make_bar)\n",
        r#"
make_bar <- function() {
    x <- list(label = "hello")
    class(x) <- "bar"
    x
}

my_format_bar <- function(x, ...) {
    paste0("bar<", x$label, ">")
}
"#,
    );

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library('barpkg')").unwrap();

    let result = s.eval_source("format(make_bar())").unwrap();
    let formatted = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(
        formatted, "bar<hello>",
        "format.bar should dispatch to my_format_bar via S3 registry"
    );
}

#[test]
fn s3method_not_in_env_chain_still_found() {
    // Verify that methods registered via S3method() are found even when
    // they are NOT exported and NOT in the calling environment chain
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();

    // print.qux is NOT exported — only registered via S3method
    create_s3_package(
        &lib_dir,
        "quxpkg",
        "S3method(print, qux)\nexport(make_qux)\n",
        r#"
make_qux <- function() {
    x <- list(n = 99)
    class(x) <- "qux"
    x
}

print.qux <- function(x, ...) {
    cat("qux:", x$n, "\n")
}
"#,
    );

    let mut s = Session::new_with_captured_output();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library('quxpkg')").unwrap();

    // print.qux should NOT be in the global env or exports
    s.eval_source(
        r#"
        stopifnot(!exists("print.qux"))
    "#,
    )
    .unwrap();

    // But S3 dispatch should still find it via the registry
    s.eval_source("print(make_qux())").unwrap();

    let output = s.captured_stdout();
    assert!(
        output.contains("qux: 99"),
        "print.qux should be found via S3 registry even though not exported, got: {output}"
    );
}

#[test]
fn s3method_use_method_dispatches_through_registry() {
    // Test that a user-defined generic using UseMethod() can find methods
    // registered via S3method() in the registry
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();

    create_s3_package(
        &lib_dir,
        "genpkg",
        "S3method(describe, widget)\nexport(describe)\nexport(make_widget)\n",
        r#"
describe <- function(x, ...) UseMethod("describe")

describe.default <- function(x, ...) "unknown"

describe.widget <- function(x, ...) {
    paste0("widget(", x$name, ")")
}

make_widget <- function(name) {
    x <- list(name = name)
    class(x) <- "widget"
    x
}
"#,
    );

    let mut s = Session::new();
    s.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();

    s.eval_source("library('genpkg')").unwrap();

    let result = s.eval_source("describe(make_widget(\"gear\"))").unwrap();
    let desc = result
        .value
        .as_vector()
        .unwrap()
        .as_character_scalar()
        .unwrap();
    assert_eq!(desc, "widget(gear)");
}

#[test]
fn s3method_registry_is_per_interpreter() {
    // Verify that the S3 method registry is per-interpreter (not global)
    let tmp = temp_dir::TempDir::new().unwrap();
    let lib_dir = tmp.path().to_path_buf();

    create_s3_package(
        &lib_dir,
        "isopkg",
        "S3method(print, iso)\nexport(make_iso)\n",
        r#"
make_iso <- function() {
    x <- list()
    class(x) <- "iso"
    x
}

print.iso <- function(x, ...) {
    cat("iso object\n")
}
"#,
    );

    // Session 1: load the package
    let mut s1 = Session::new_with_captured_output();
    s1.eval_source(&format!("Sys.setenv(R_LIBS = \"{}\")", lib_dir.display()))
        .unwrap();
    s1.eval_source("library('isopkg')").unwrap();

    // Session 2: does NOT load the package
    let mut s2 = Session::new_with_captured_output();

    // s1 should be able to dispatch print.iso
    s1.eval_source("print(make_iso())").unwrap();
    let out1 = s1.captured_stdout();
    assert!(
        out1.contains("iso object"),
        "s1 should dispatch print.iso: {out1}"
    );

    // s2 should NOT know about print.iso — print() should use default
    // list printing, not the package's print.iso method
    s2.eval_source(
        r#"
        obj <- list()
        class(obj) <- "iso"
        print(obj)
    "#,
    )
    .unwrap();

    let out2 = s2.captured_stdout();
    assert!(
        !out2.contains("iso object"),
        "s2 should NOT dispatch print.iso (per-interpreter isolation), got: {out2}"
    );
}

// endregion
