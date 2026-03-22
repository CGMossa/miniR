#![cfg(feature = "pdf-device")]

//! Integration tests for the PDF graphics device (krilla-svg backend).

use r::Session;

fn temp_dir() -> temp_dir::TempDir {
    temp_dir::TempDir::new().unwrap()
}

/// Verify that `pdf(); plot(1:10); dev.off()` produces a file starting with `%PDF`.
#[test]
fn pdf_output_has_real_pdf_header() {
    let dir = temp_dir();
    let pdf_path = dir.path().join("test.pdf");
    let pdf_path_str = pdf_path.to_str().unwrap().replace('\\', "/");

    let mut s = Session::new();
    s.eval_source(&format!(r#"pdf("{pdf_path_str}")"#)).unwrap();
    s.eval_source("plot(1:10)").unwrap();
    s.eval_source("dev.off()").unwrap();

    assert!(
        pdf_path.exists(),
        "PDF file was not created at {pdf_path_str}"
    );
    let contents = std::fs::read(&pdf_path).unwrap();
    assert!(
        contents.len() > 4,
        "PDF file is too small ({} bytes)",
        contents.len()
    );
    assert_eq!(
        &contents[..5],
        b"%PDF-",
        "file does not start with PDF header; got {:?}",
        String::from_utf8_lossy(&contents[..std::cmp::min(20, contents.len())])
    );
}

/// Verify that `svg(); plot(1:10); dev.off()` produces a valid SVG file.
#[test]
fn svg_output_has_svg_header() {
    let dir = temp_dir();
    let svg_path = dir.path().join("test.svg");
    let svg_path_str = svg_path.to_str().unwrap().replace('\\', "/");

    let mut s = Session::new();
    s.eval_source(&format!(r#"svg("{svg_path_str}")"#)).unwrap();
    s.eval_source("plot(1:10)").unwrap();
    s.eval_source("dev.off()").unwrap();

    assert!(svg_path.exists(), "SVG file was not created");
    let contents = std::fs::read_to_string(&svg_path).unwrap();
    assert!(
        contents.contains("<svg"),
        "file does not contain SVG markup"
    );
}

/// Verify that dev.off() with no device open returns 1 without error.
#[test]
fn dev_off_no_device_returns_null_device() {
    let mut s = Session::new();
    let result = s.eval_source("dev.off()").unwrap();
    let val = result.value;
    if let r::interpreter::value::RValue::Vector(rv) = &val {
        if let Some(i) = rv.inner.as_integer_scalar() {
            assert_eq!(i, 1, "dev.off() with no device should return 1");
        } else {
            panic!("dev.off() should return an integer, got: {val:?}");
        }
    } else {
        panic!("dev.off() should return a vector, got: {val:?}");
    }
}

/// Verify that `pdf(); plot(1:10, type="l"); dev.off()` works for line plots.
#[test]
fn pdf_line_plot() {
    let dir = temp_dir();
    let pdf_path = dir.path().join("lines.pdf");
    let pdf_path_str = pdf_path.to_str().unwrap().replace('\\', "/");

    let mut s = Session::new();
    s.eval_source(&format!(r#"pdf("{pdf_path_str}")"#)).unwrap();
    s.eval_source(r#"plot(1:10, type="l")"#).unwrap();
    s.eval_source("dev.off()").unwrap();

    assert!(pdf_path.exists(), "PDF file was not created");
    let contents = std::fs::read(&pdf_path).unwrap();
    assert_eq!(&contents[..5], b"%PDF-");
}

/// Verify that plot with x and y vectors works.
#[test]
fn pdf_xy_plot() {
    let dir = temp_dir();
    let pdf_path = dir.path().join("xy.pdf");
    let pdf_path_str = pdf_path.to_str().unwrap().replace('\\', "/");

    let mut s = Session::new();
    s.eval_source(&format!(r#"pdf("{pdf_path_str}")"#)).unwrap();
    s.eval_source("plot(c(1,2,3,4,5), c(10,20,30,40,50))")
        .unwrap();
    s.eval_source("dev.off()").unwrap();

    assert!(pdf_path.exists());
    let contents = std::fs::read(&pdf_path).unwrap();
    assert_eq!(&contents[..5], b"%PDF-");
}

/// Verify title and labels are included in SVG output.
#[test]
fn svg_title_and_labels() {
    let dir = temp_dir();
    let svg_path = dir.path().join("labeled.svg");
    let svg_path_str = svg_path.to_str().unwrap().replace('\\', "/");

    let mut s = Session::new();
    s.eval_source(&format!(r#"svg("{svg_path_str}")"#)).unwrap();
    s.eval_source(r#"plot(1:5, main="Test Title", xlab="X Axis", ylab="Y Axis")"#)
        .unwrap();
    s.eval_source("dev.off()").unwrap();

    let contents = std::fs::read_to_string(&svg_path).unwrap();
    assert!(
        contents.contains("Test Title"),
        "SVG should contain main title"
    );
    assert!(
        contents.contains("X Axis"),
        "SVG should contain x-axis label"
    );
    assert!(
        contents.contains("Y Axis"),
        "SVG should contain y-axis label"
    );
}
