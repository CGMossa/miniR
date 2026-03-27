//! Tests for the native code loading pipeline:
//! Makevars parsing, C compilation, dyn.load, and .Call dispatch.

#![cfg(feature = "native")]
#![allow(unused_must_use)]

use r::Session;
use std::io::Write;
use std::path::PathBuf;

/// Get the path to miniR's include/ directory (for Rinternals.h).
fn include_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include")
}

/// Create a temporary directory with a C source file.
fn make_test_package(c_code: &str) -> (temp_dir::TempDir, PathBuf) {
    let tmp = temp_dir::TempDir::new().expect("create temp dir");
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    let c_file = src_dir.join("test.c");
    let mut f = std::fs::File::create(&c_file).expect("create test.c");
    f.write_all(c_code.as_bytes()).expect("write test.c");

    (tmp, src_dir)
}

// region: Makevars parsing tests

#[test]
fn makevars_parse_simple() {
    use r::interpreter::native::compile::Makevars;

    let mv = Makevars::parse_str("PKG_CFLAGS = -Wall -O2\nPKG_LIBS = -lz\n");
    assert_eq!(mv.pkg_cflags(), "-Wall -O2");
    assert_eq!(mv.pkg_libs(), "-lz");
}

#[test]
fn makevars_parse_continuation() {
    use r::interpreter::native::compile::Makevars;

    let mv = Makevars::parse_str("PKG_CFLAGS = -Wall \\\n  -O2\n");
    assert_eq!(mv.pkg_cflags(), "-Wall -O2");
}

#[test]
fn makevars_parse_objects() {
    use r::interpreter::native::compile::Makevars;

    let mv = Makevars::parse_str("OBJECTS = foo.o bar.o baz.o\n");
    assert_eq!(mv.objects(), Some("foo.o bar.o baz.o"));
}

// endregion

// region: C compilation tests

#[test]
fn compile_simple_c_file() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_double_it(SEXP x) {
    int n = LENGTH(x);
    SEXP result = PROTECT(Rf_allocVector(REALSXP, n));
    double *px = REAL(x);
    double *pr = REAL(result);
    for (int i = 0; i < n; i++) {
        pr[i] = px[i] * 2.0;
    }
    UNPROTECT(1);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let result = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir());
    assert!(result.is_ok(), "compilation failed: {:?}", result.err());

    let lib_path = result.expect("lib path");
    assert!(lib_path.is_file(), "shared library not created");
}

// endregion

// region: dyn.load + .Call integration tests

#[test]
fn dot_call_double_vector() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_double_it(SEXP x) {
    int n = LENGTH(x);
    SEXP result = PROTECT(Rf_allocVector(REALSXP, n));
    double *px = REAL(x);
    double *pr = REAL(result);
    for (int i = 0; i < n; i++) {
        pr[i] = px[i] * 2.0;
    }
    UNPROTECT(1);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    // Load the compiled library
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    // Call the native function
    s.eval_source(
        r#"
result <- .Call("test_double_it", c(1.0, 2.0, 3.0))
stopifnot(identical(result, c(2.0, 4.0, 6.0)))
"#,
    );
}

#[test]
fn dot_call_integer_vector() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_sum_ints(SEXP x) {
    int n = LENGTH(x);
    int *px = INTEGER(x);
    int total = 0;
    for (int i = 0; i < n; i++) {
        total += px[i];
    }
    return Rf_ScalarInteger(total);
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_sum_ints", 1:5)
stopifnot(result == 15L)
"#,
    );
}

#[test]
fn dot_call_string_vector() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>
#include <string.h>

SEXP test_string_length(SEXP x) {
    int n = LENGTH(x);
    SEXP result = PROTECT(Rf_allocVector(INTSXP, n));
    int *pr = INTEGER(result);
    for (int i = 0; i < n; i++) {
        SEXP elt = STRING_ELT(x, i);
        pr[i] = (int)strlen(R_CHAR(elt));
    }
    UNPROTECT(1);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_string_length", c("hello", "ab", "miniR"))
stopifnot(identical(result, c(5L, 2L, 5L)))
"#,
    );
}

#[test]
fn dot_call_logical_vector() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_negate_logical(SEXP x) {
    int n = LENGTH(x);
    SEXP result = PROTECT(Rf_allocVector(LGLSXP, n));
    int *px = LOGICAL(x);
    int *pr = LOGICAL(result);
    for (int i = 0; i < n; i++) {
        if (px[i] == NA_LOGICAL) {
            pr[i] = NA_LOGICAL;
        } else {
            pr[i] = !px[i];
        }
    }
    UNPROTECT(1);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_negate_logical", c(TRUE, FALSE, TRUE))
stopifnot(identical(result, c(FALSE, TRUE, FALSE)))
"#,
    );
}

#[test]
fn dot_call_return_string() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_hello(void) {
    return Rf_mkString("hello from C");
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_hello")
stopifnot(result == "hello from C")
"#,
    );
}

#[test]
fn dot_call_scalar_real() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_add_one(SEXP x) {
    double val = REAL(x)[0];
    return Rf_ScalarReal(val + 1.0);
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_add_one", 41.0)
stopifnot(result == 42.0)
"#,
    );
}

#[test]
fn dyn_unload_works() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>
SEXP test_one(void) { return Rf_ScalarInteger(1); }
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));
    s.eval_source("stopifnot(is.loaded(\"test_one\"))");
    s.eval_source(&format!("dyn.unload(\"{}\")", lib_path.display()));
    s.eval_source("stopifnot(!is.loaded(\"test_one\"))");
}

#[test]
fn dot_call_with_na_values() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_count_na(SEXP x) {
    int n = LENGTH(x);
    double *px = REAL(x);
    int count = 0;
    for (int i = 0; i < n; i++) {
        if (ISNA(px[i])) count++;
    }
    return Rf_ScalarInteger(count);
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_count_na", c(1.0, NA, 3.0, NA))
stopifnot(result == 2L)
"#,
    );
}

// endregion

// region: Rf_error handling (setjmp trampoline)

#[test]
fn dot_call_rf_error_returns_r_error() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_error(SEXP x) {
    if (LENGTH(x) == 0) {
        Rf_error("input vector must not be empty");
    }
    return x;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    // Should succeed with non-empty vector
    s.eval_source(
        r#"
result <- .Call("test_error", 1:3)
stopifnot(identical(result, 1:3))
"#,
    );

    // Should fail with empty vector (Rf_error)
    let result = s.eval_source(r#".Call("test_error", integer(0))"#);
    assert!(result.is_err(), "expected Rf_error to propagate as RError");
}

// endregion

// region: R_RegisterRoutines

#[test]
fn r_register_routines() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP my_registered_fn(SEXP x) {
    return Rf_ScalarInteger(LENGTH(x) * 10);
}

static const R_CallMethodDef callMethods[] = {
    {"my_registered_fn", (void*)&my_registered_fn, 1},
    {NULL, NULL, 0}
};

void R_init_testpkg(DllInfo *info) {
    R_registerRoutines(info, NULL, callMethods, NULL, NULL);
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    // The function should be callable by its registered name
    s.eval_source(
        r#"
result <- .Call("my_registered_fn", 1:5)
stopifnot(result == 50L)
"#,
    );
}

// endregion

// region: Complex vector support

#[test]
fn dot_call_complex_vector() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_complex_sum(SEXP x) {
    int n = LENGTH(x);
    Rcomplex *px = COMPLEX(x);
    double re = 0.0, im = 0.0;
    for (int i = 0; i < n; i++) {
        re += px[i].r;
        im += px[i].i;
    }
    SEXP result = PROTECT(Rf_allocVector(CPLXSXP, 1));
    COMPLEX(result)[0].r = re;
    COMPLEX(result)[0].i = im;
    UNPROTECT(1);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_complex_sum", c(1+2i, 3+4i))
stopifnot(Re(result) == 4)
stopifnot(Im(result) == 6)
"#,
    );
}

// endregion

// region: Attributes

#[test]
fn dot_call_set_names_attribute() {
    use r::interpreter::native::compile;

    let c_code = r#"
#include <Rinternals.h>

SEXP test_with_names(SEXP x) {
    int n = LENGTH(x);
    SEXP result = PROTECT(Rf_duplicate(x));
    SEXP names = PROTECT(Rf_allocVector(STRSXP, n));
    for (int i = 0; i < n; i++) {
        char buf[32];
        snprintf(buf, sizeof(buf), "v%d", i + 1);
        SET_STRING_ELT(names, i, Rf_mkChar(buf));
    }
    Rf_setAttrib(result, R_NamesSymbol, names);
    UNPROTECT(2);
    return result;
}
"#;

    let (tmp, src_dir) = make_test_package(c_code);
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "testpkg", &output_dir, &include_dir())
        .expect("compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
result <- .Call("test_with_names", c(10, 20, 30))
stopifnot(identical(names(result), c("v1", "v2", "v3")))
stopifnot(identical(as.numeric(result), c(10, 20, 30)))
"#,
    );
}

// endregion

// region: Multi-file compilation (tests that runtime.c is linked once)

#[test]
fn compile_multi_file_package() {
    use r::interpreter::native::compile;

    // Write two .c files — both include Rinternals.h
    let tmp = temp_dir::TempDir::new().expect("create temp dir");
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    std::fs::write(
        src_dir.join("a.c"),
        r#"
#include <Rinternals.h>
SEXP from_a(void) { return Rf_ScalarInteger(1); }
"#,
    )
    .expect("write a.c");

    std::fs::write(
        src_dir.join("b.c"),
        r#"
#include <Rinternals.h>
SEXP from_b(void) { return Rf_ScalarInteger(2); }
"#,
    )
    .expect("write b.c");

    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    let lib_path = compile::compile_package(&src_dir, "multipkg", &output_dir, &include_dir())
        .expect("multi-file compilation failed");

    let mut s = Session::new();
    s.eval_source(&format!("dyn.load(\"{}\")", lib_path.display()));

    s.eval_source(
        r#"
stopifnot(.Call("from_a") == 1L)
stopifnot(.Call("from_b") == 2L)
"#,
    );
}

// endregion
