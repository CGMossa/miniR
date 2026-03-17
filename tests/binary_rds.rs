//! Tests for reading GNU R binary RDS files.

use r::Session;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("minir-binrds-{name}-{suffix}.rds"))
}

fn quote_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

// region: XDR byte building helpers

/// Build a complete XDR binary RDS stream for a single object.
struct RdsBuilder {
    buf: Vec<u8>,
}

impl RdsBuilder {
    fn new() -> Self {
        let mut b = RdsBuilder { buf: Vec::new() };
        // Format header
        b.buf.extend_from_slice(b"X\n");
        // Version 2
        b.write_i32(2);
        // R version that wrote (4.3.0 = 0x00040300 as packed)
        b.write_i32(0x00040300);
        // Minimum R version to read (2.3.0)
        b.write_i32(0x00020300);
        b
    }

    fn write_i32(&mut self, val: i32) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    fn write_f64(&mut self, val: f64) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    fn write_flags(&mut self, sxp_type: u8, has_attr: bool, has_tag: bool) {
        let mut flags: u32 = u32::from(sxp_type);
        if has_attr {
            flags |= 1 << 9;
        }
        if has_tag {
            flags |= 1 << 10;
        }
        self.write_i32(flags as i32);
    }

    fn write_charsxp(&mut self, s: &str) {
        // CHARSXP flags: type 9, encoding bits in gp field
        // gp = 1 << 12 for native/UTF-8 encoding
        let flags: u32 = 9 | (1 << 12);
        self.write_i32(flags as i32);
        self.write_i32(s.len() as i32);
        self.buf.extend_from_slice(s.as_bytes());
    }

    fn write_na_charsxp(&mut self) {
        let flags: u32 = 9;
        self.write_i32(flags as i32);
        self.write_i32(-1); // NA_STRING
    }

    fn write_nilvalue(&mut self) {
        self.write_flags(254, false, false);
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

// endregion

// region: double vector tests

#[test]
fn read_binary_rds_double_vector() {
    let path = temp_path("double-vec");
    let mut b = RdsBuilder::new();

    // REALSXP (14), no attributes
    b.write_flags(14, false, false);
    b.write_i32(3); // length
    b.write_f64(1.0);
    b.write_f64(2.5);
    b.write_f64(3.0);

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.double(x))
stopifnot(length(x) == 3)
stopifnot(x[1] == 1.0)
stopifnot(x[2] == 2.5)
stopifnot(x[3] == 3.0)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

#[test]
fn read_binary_rds_double_with_na() {
    let path = temp_path("double-na");
    let mut b = RdsBuilder::new();

    b.write_flags(14, false, false);
    b.write_i32(2);
    b.write_f64(42.0);
    // NA_real: 0x7FF00000000007A2
    b.buf
        .extend_from_slice(&0x7FF00000000007A2u64.to_be_bytes());

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(x[1] == 42.0)
stopifnot(is.na(x[2]))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: integer vector tests

#[test]
fn read_binary_rds_integer_vector() {
    let path = temp_path("int-vec");
    let mut b = RdsBuilder::new();

    // INTSXP (13)
    b.write_flags(13, false, false);
    b.write_i32(3);
    b.write_i32(10);
    b.write_i32(20);
    b.write_i32(30);

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.integer(x))
stopifnot(length(x) == 3)
stopifnot(x[1] == 10L)
stopifnot(x[2] == 20L)
stopifnot(x[3] == 30L)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

#[test]
fn read_binary_rds_integer_with_na() {
    let path = temp_path("int-na");
    let mut b = RdsBuilder::new();

    b.write_flags(13, false, false);
    b.write_i32(2);
    b.write_i32(5);
    b.write_i32(i32::MIN); // NA_integer

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(x[1] == 5L)
stopifnot(is.na(x[2]))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: logical vector tests

#[test]
fn read_binary_rds_logical_vector() {
    let path = temp_path("lgl-vec");
    let mut b = RdsBuilder::new();

    // LGLSXP (10)
    b.write_flags(10, false, false);
    b.write_i32(3);
    b.write_i32(1); // TRUE
    b.write_i32(0); // FALSE
    b.write_i32(i32::MIN); // NA

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.logical(x))
stopifnot(length(x) == 3)
stopifnot(x[1] == TRUE)
stopifnot(x[2] == FALSE)
stopifnot(is.na(x[3]))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: character vector tests

#[test]
fn read_binary_rds_character_vector() {
    let path = temp_path("chr-vec");
    let mut b = RdsBuilder::new();

    // STRSXP (16)
    b.write_flags(16, false, false);
    b.write_i32(3); // 3 elements
    b.write_charsxp("hello");
    b.write_charsxp("world");
    b.write_na_charsxp(); // NA_character_

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.character(x))
stopifnot(length(x) == 3)
stopifnot(x[1] == "hello")
stopifnot(x[2] == "world")
stopifnot(is.na(x[3]))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: raw vector tests

#[test]
fn read_binary_rds_raw_vector() {
    let path = temp_path("raw-vec");
    let mut b = RdsBuilder::new();

    // RAWSXP (24)
    b.write_flags(24, false, false);
    b.write_i32(4);
    b.buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.raw(x))
stopifnot(length(x) == 4)
stopifnot(x[1] == as.raw(0xDE))
stopifnot(x[2] == as.raw(0xAD))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: complex vector tests

#[test]
fn read_binary_rds_complex_vector() {
    let path = temp_path("cplx-vec");
    let mut b = RdsBuilder::new();

    // CPLXSXP (15)
    b.write_flags(15, false, false);
    b.write_i32(2);
    b.write_f64(1.0); // re
    b.write_f64(2.0); // im
    b.write_f64(3.0); // re
    b.write_f64(-4.0); // im

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.complex(x))
stopifnot(length(x) == 2)
stopifnot(Re(x[1]) == 1.0)
stopifnot(Im(x[1]) == 2.0)
stopifnot(Re(x[2]) == 3.0)
stopifnot(Im(x[2]) == -4.0)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: NULL tests

#[test]
fn read_binary_rds_null() {
    let path = temp_path("null");
    let mut b = RdsBuilder::new();
    b.write_flags(254, false, false); // NILVALUE_SXP

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.null(x))
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: named vector (attributes) tests

#[test]
fn read_binary_rds_named_integer_vector() {
    let path = temp_path("named-int");
    let mut b = RdsBuilder::new();

    // INTSXP (13) with attributes
    b.write_flags(13, true, false);
    b.write_i32(3);
    b.write_i32(10);
    b.write_i32(20);
    b.write_i32(30);

    // Attributes pairlist: names = c("a", "b", "c")
    // LISTSXP node with tag
    b.write_flags(2, false, true); // LISTSXP, has_tag
                                   // Tag: symbol "names"
    b.write_flags(1, false, false); // SYMSXP
    b.write_charsxp("names");
    // Value: STRSXP c("a", "b", "c")
    b.write_flags(16, false, false);
    b.write_i32(3);
    b.write_charsxp("a");
    b.write_charsxp("b");
    b.write_charsxp("c");
    // CDR: NILVALUE (end of pairlist)
    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let p = quote_path(&path);
    s.eval_source(&format!(r#"x <- readRDS("{p}")"#))
        .expect("readRDS failed");
    s.eval_source("stopifnot(is.integer(x))")
        .expect("is.integer check failed");
    s.eval_source(r#"stopifnot(identical(names(x), c("a", "b", "c")))"#)
        .expect("names check failed");
    s.eval_source(r#"stopifnot(x["a"] == 10L)"#)
        .expect("x[a] check failed");
    s.eval_source(r#"stopifnot(x["c"] == 30L)"#)
        .expect("x[c] check failed");

    let _ = fs::remove_file(path);
}

// endregion

// region: list (VECSXP) tests

#[test]
fn read_binary_rds_list() {
    let path = temp_path("list");
    let mut b = RdsBuilder::new();

    // VECSXP (19) with names attribute
    b.write_flags(19, true, false);
    b.write_i32(2); // 2 elements

    // Element 1: integer scalar 42
    b.write_flags(13, false, false);
    b.write_i32(1);
    b.write_i32(42);

    // Element 2: character scalar "hello"
    b.write_flags(16, false, false);
    b.write_i32(1);
    b.write_charsxp("hello");

    // Attributes pairlist: names = c("x", "y")
    b.write_flags(2, false, true);
    b.write_flags(1, false, false); // SYMSXP
    b.write_charsxp("names");
    b.write_flags(16, false, false); // STRSXP
    b.write_i32(2);
    b.write_charsxp("x");
    b.write_charsxp("y");
    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.list(x))
stopifnot(length(x) == 2)
stopifnot(x$x == 42L)
stopifnot(x$y == "hello")
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: data.frame tests

#[test]
fn read_binary_rds_data_frame() {
    // A data.frame is a VECSXP with class="data.frame", names, and row.names attributes.
    let path = temp_path("dataframe");
    let mut b = RdsBuilder::new();

    // VECSXP (19) with attributes
    b.write_flags(19, true, false);
    b.write_i32(2); // 2 columns

    // Column 1: integer c(1, 2, 3)
    b.write_flags(13, false, false);
    b.write_i32(3);
    b.write_i32(1);
    b.write_i32(2);
    b.write_i32(3);

    // Column 2: character c("a", "b", "c")
    b.write_flags(16, false, false);
    b.write_i32(3);
    b.write_charsxp("a");
    b.write_charsxp("b");
    b.write_charsxp("c");

    // Attributes pairlist: names, class, row.names
    // 1. names = c("x", "y")
    b.write_flags(2, false, true);
    b.write_flags(1, false, false);
    b.write_charsxp("names");
    b.write_flags(16, false, false);
    b.write_i32(2);
    b.write_charsxp("x");
    b.write_charsxp("y");

    // 2. class = "data.frame"
    b.write_flags(2, false, true);
    b.write_flags(1, false, false);
    b.write_charsxp("class");
    b.write_flags(16, false, false);
    b.write_i32(1);
    b.write_charsxp("data.frame");

    // 3. row.names = c("1", "2", "3") (explicit string form)
    b.write_flags(2, false, true);
    b.write_flags(1, false, false);
    b.write_charsxp("row.names");
    b.write_flags(16, false, false);
    b.write_i32(3);
    b.write_charsxp("1");
    b.write_charsxp("2");
    b.write_charsxp("3");

    // End of attributes
    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let p = quote_path(&path);
    s.eval_source(&format!(r#"df <- readRDS("{p}")"#))
        .expect("readRDS failed");
    s.eval_source("stopifnot(is.data.frame(df))")
        .expect("is.data.frame check failed");
    s.eval_source(r#"stopifnot(identical(names(df), c("x", "y")))"#)
        .expect("names check failed");
    s.eval_source("stopifnot(nrow(df) == 3)")
        .expect("nrow check failed");
    s.eval_source("stopifnot(df$x[1] == 1L)")
        .expect("df$x[1] check failed");
    s.eval_source(r#"stopifnot(df$y[2] == "b")"#)
        .expect("df$y[2] check failed");

    let _ = fs::remove_file(path);
}

// endregion

// region: gzip compression tests

#[cfg(feature = "compression")]
#[test]
fn read_binary_rds_gzip_compressed() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let path = temp_path("gzip");
    let mut b = RdsBuilder::new();

    // Simple integer vector c(1L, 2L, 3L)
    b.write_flags(13, false, false);
    b.write_i32(3);
    b.write_i32(1);
    b.write_i32(2);
    b.write_i32(3);

    let uncompressed = b.finish();

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&uncompressed).unwrap();
    let compressed = encoder.finish().unwrap();

    fs::write(&path, compressed).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.integer(x))
stopifnot(length(x) == 3)
stopifnot(x[1] == 1L)
stopifnot(x[2] == 2L)
stopifnot(x[3] == 3L)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: version 3 tests

#[test]
fn read_binary_rds_version3() {
    // Version 3 has an additional native encoding string after the version triple.
    let path = temp_path("v3");
    let mut buf = Vec::new();

    // Format header
    buf.extend_from_slice(b"X\n");
    // Version 3
    buf.extend_from_slice(&3i32.to_be_bytes());
    // R version wrote
    buf.extend_from_slice(&0x00040300i32.to_be_bytes());
    // R version min
    buf.extend_from_slice(&0x00030500i32.to_be_bytes());
    // Native encoding: STRSXP of length 1 containing "UTF-8"
    // STRSXP flags
    buf.extend_from_slice(&16i32.to_be_bytes());
    // length 1
    buf.extend_from_slice(&1i32.to_be_bytes());
    // CHARSXP for "UTF-8"
    let charsxp_flags: u32 = 9 | (1 << 12);
    buf.extend_from_slice(&(charsxp_flags as i32).to_be_bytes());
    buf.extend_from_slice(&5i32.to_be_bytes());
    buf.extend_from_slice(b"UTF-8");

    // Now the actual object: double scalar 99.5
    // REALSXP flags
    buf.extend_from_slice(&14i32.to_be_bytes());
    buf.extend_from_slice(&1i32.to_be_bytes());
    buf.extend_from_slice(&99.5f64.to_be_bytes());

    fs::write(&path, buf).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(x == 99.5)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: reference object tests

#[test]
fn read_binary_rds_with_symbol_references() {
    // Test that symbol references (REFSXP) work correctly.
    // Create a named list where the "names" attribute symbol is referenced.
    let path = temp_path("symref");
    let mut b = RdsBuilder::new();

    // VECSXP with names attribute
    b.write_flags(19, true, false);
    b.write_i32(2);

    // Two integer scalars
    b.write_flags(13, false, false);
    b.write_i32(1);
    b.write_i32(100);

    b.write_flags(13, false, false);
    b.write_i32(1);
    b.write_i32(200);

    // Attributes: names = c("a", "b")
    b.write_flags(2, false, true);
    // Tag: SYMSXP for "names" (will be added to ref table as item #1)
    b.write_flags(1, false, false);
    b.write_charsxp("names");
    // Value: STRSXP
    b.write_flags(16, false, false);
    b.write_i32(2);
    b.write_charsxp("a");
    b.write_charsxp("b");
    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.list(x))
stopifnot(x$a == 100L)
stopifnot(x$b == 200L)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: empty vector tests

#[test]
fn read_binary_rds_empty_vectors() {
    let path = temp_path("empty-vec");
    let mut b = RdsBuilder::new();

    // Empty double vector
    b.write_flags(14, false, false);
    b.write_i32(0);

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
x <- readRDS("{path}")
stopifnot(is.double(x))
stopifnot(length(x) == 0)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: matrix tests

#[test]
fn read_binary_rds_matrix() {
    // A matrix is a vector with dim attribute.
    let path = temp_path("matrix");
    let mut b = RdsBuilder::new();

    // REALSXP with dim attribute
    b.write_flags(14, true, false);
    b.write_i32(4); // 2x2 matrix = 4 elements
    b.write_f64(1.0);
    b.write_f64(2.0);
    b.write_f64(3.0);
    b.write_f64(4.0);

    // Attributes: dim = c(2L, 2L)
    b.write_flags(2, false, true);
    b.write_flags(1, false, false); // SYMSXP
    b.write_charsxp("dim");
    b.write_flags(13, false, false); // INTSXP
    b.write_i32(2); // length 2
    b.write_i32(2); // nrow
    b.write_i32(2); // ncol
    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
m <- readRDS("{path}")
stopifnot(is.matrix(m))
stopifnot(nrow(m) == 2)
stopifnot(ncol(m) == 2)
stopifnot(m[1,1] == 1.0)
stopifnot(m[2,1] == 2.0)
stopifnot(m[1,2] == 3.0)
stopifnot(m[2,2] == 4.0)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: factor tests

#[test]
fn read_binary_rds_factor() {
    // A factor is an INTSXP with class="factor" and levels attribute.
    let path = temp_path("factor");
    let mut b = RdsBuilder::new();

    // INTSXP with attributes
    b.write_flags(13, true, false);
    b.write_i32(3); // 3 elements
    b.write_i32(2); // index into levels (1-based)
    b.write_i32(1);
    b.write_i32(2);

    // Attributes: levels, class
    // 1. levels = c("a", "b")
    b.write_flags(2, false, true);
    b.write_flags(1, false, false);
    b.write_charsxp("levels");
    b.write_flags(16, false, false);
    b.write_i32(2);
    b.write_charsxp("a");
    b.write_charsxp("b");

    // 2. class = "factor"
    b.write_flags(2, false, true);
    b.write_flags(1, false, false);
    b.write_charsxp("class");
    b.write_flags(16, false, false);
    b.write_i32(1);
    b.write_charsxp("factor");

    b.write_nilvalue();

    fs::write(&path, b.finish()).unwrap();

    let mut s = Session::new();
    let script = format!(
        r#"
f <- readRDS("{path}")
stopifnot(is.factor(f))
stopifnot(identical(levels(f), c("a", "b")))
stopifnot(length(f) == 3)
"#,
        path = quote_path(&path)
    );
    s.eval_source(&script).unwrap();

    let _ = fs::remove_file(path);
}

// endregion

// region: error handling tests

#[test]
fn read_binary_rds_ascii_format_gives_clear_error() {
    let path = temp_path("ascii-fmt");
    let mut data = Vec::new();
    data.extend_from_slice(b"A\n");
    // Some version bytes
    data.extend_from_slice(&2i32.to_be_bytes());
    data.extend_from_slice(&0x00040300i32.to_be_bytes());
    data.extend_from_slice(&0x00020300i32.to_be_bytes());

    fs::write(&path, data).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_r"))
        .args(["-e", &format!("readRDS(\"{}\")", quote_path(&path))])
        .output()
        .expect("failed to run miniR");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ASCII") || stderr.contains("ascii"),
        "expected ASCII format error, got: {stderr}"
    );

    let _ = fs::remove_file(path);
}

// endregion

// region: XDR writer round-trip tests

#[test]
fn roundtrip_xdr_integer_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-int");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1L, 2L, NA_integer_, 4L)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(identical(x, y))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_double_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-dbl");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1.5, NA_real_, -Inf, 0.0)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.double(y))
stopifnot(y[1] == 1.5)
stopifnot(is.na(y[2]))
stopifnot(y[3] == -Inf)
stopifnot(y[4] == 0.0)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_logical_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-lgl");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(TRUE, FALSE, NA)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.logical(y))
stopifnot(y[1] == TRUE)
stopifnot(y[2] == FALSE)
stopifnot(is.na(y[3]))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_character_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-chr");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c("hello", NA_character_, "world", "")
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.character(y))
stopifnot(y[1] == "hello")
stopifnot(is.na(y[2]))
stopifnot(y[3] == "world")
stopifnot(y[4] == "")
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_complex_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-cplx");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1+2i, 3-4i, NA_complex_)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.complex(y))
stopifnot(Re(y[1]) == 1)
stopifnot(Im(y[1]) == 2)
stopifnot(Re(y[2]) == 3)
stopifnot(Im(y[2]) == -4)
stopifnot(is.na(y[3]))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_raw_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-raw");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- as.raw(c(0, 255, 42))
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.raw(y))
stopifnot(length(y) == 3)
stopifnot(y[1] == as.raw(0))
stopifnot(y[2] == as.raw(255))
stopifnot(y[3] == as.raw(42))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_null() {
    let mut s = Session::new();
    let path = temp_path("rt-null");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
saveRDS(NULL, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.null(y))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_named_vector() {
    let mut s = Session::new();
    let path = temp_path("rt-named");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(a = 1L, b = 2L, c = 3L)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.integer(y))
stopifnot(identical(names(y), c("a", "b", "c")))
stopifnot(y[["a"]] == 1L)
stopifnot(y[["c"]] == 3L)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_list_with_names() {
    let mut s = Session::new();
    let path = temp_path("rt-list");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- list(a = 1L, b = "hello", c = TRUE)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.list(y))
stopifnot(length(y) == 3)
stopifnot(y$a == 1L)
stopifnot(y$b == "hello")
stopifnot(y$c == TRUE)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_nested_list() {
    let mut s = Session::new();
    let path = temp_path("rt-nested");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- list(inner = list(a = 1L, b = 2L), value = 42.0)
saveRDS(x, "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.list(y))
stopifnot(y$inner$a == 1L)
stopifnot(y$inner$b == 2L)
stopifnot(y$value == 42.0)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_empty_vectors() {
    let mut s = Session::new();
    let path = temp_path("rt-empty");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
saveRDS(integer(0), "{p}", compress = FALSE)
y <- readRDS("{p}")
stopifnot(is.integer(y))
stopifnot(length(y) == 0)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_data_frame() {
    let mut s = Session::new();
    let path = temp_path("rt-df");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
df <- data.frame(x = c(1L, 2L), y = c("a", "b"))
saveRDS(df, "{p}", compress = FALSE)
df2 <- readRDS("{p}")
stopifnot(is.data.frame(df2))
stopifnot(identical(names(df2), c("x", "y")))
stopifnot(nrow(df2) == 2)
stopifnot(df2$x[1] == 1L)
stopifnot(df2$y[2] == "b")
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[cfg(feature = "compression")]
#[test]
fn roundtrip_xdr_gzip_compressed() {
    let mut s = Session::new();
    let path = temp_path("rt-gz");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
x <- c(1L, 2L, 3L)
saveRDS(x, "{p}", compress = TRUE)
y <- readRDS("{p}")
stopifnot(is.integer(y))
stopifnot(identical(x, y))
"#
    ))
    .unwrap();

    // Verify the file is actually gzip-compressed (magic bytes 0x1f 0x8b)
    let bytes = fs::read(&path).unwrap();
    assert!(
        bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b,
        "expected gzip-compressed output, first bytes: {:?}",
        &bytes[..bytes.len().min(4)]
    );

    let _ = fs::remove_file(path);
}

#[cfg(feature = "compression")]
#[test]
fn roundtrip_xdr_compress_false_is_uncompressed() {
    let mut s = Session::new();
    let path = temp_path("rt-nocomp");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
saveRDS(42L, "{p}", compress = FALSE)
"#
    ))
    .unwrap();

    // Should start with "X\n", not gzip magic
    let bytes = fs::read(&path).unwrap();
    assert!(
        bytes.len() >= 2 && bytes[0] == b'X' && bytes[1] == b'\n',
        "expected uncompressed XDR header 'X\\n', first bytes: {:?}",
        &bytes[..bytes.len().min(4)]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_matrix_with_dimnames() {
    let mut s = Session::new();
    let path = temp_path("rt-matrix");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
m <- matrix(1:4, nrow = 2, dimnames = list(c("r1", "r2"), c("c1", "c2")))
saveRDS(m, "{p}", compress = FALSE)
m2 <- readRDS("{p}")
stopifnot(is.matrix(m2))
stopifnot(nrow(m2) == 2)
stopifnot(ncol(m2) == 2)
stopifnot(identical(rownames(m2), c("r1", "r2")))
stopifnot(identical(colnames(m2), c("c1", "c2")))
stopifnot(m2[1,1] == 1L)
stopifnot(m2[2,2] == 4L)
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

#[test]
fn roundtrip_xdr_factor() {
    let mut s = Session::new();
    let path = temp_path("rt-factor");
    let p = quote_path(&path);
    s.eval_source(&format!(
        r#"
f <- factor(c("b", "a", "b"))
saveRDS(f, "{p}", compress = FALSE)
f2 <- readRDS("{p}")
stopifnot(is.factor(f2))
stopifnot(identical(levels(f2), levels(f)))
stopifnot(identical(as.integer(f2), as.integer(f)))
"#
    ))
    .unwrap();
    let _ = fs::remove_file(path);
}

// endregion
