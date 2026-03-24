//! Integration tests for the SVG file device.
//!
//! Verifies that `svg("out.svg"); plot(1:10); dev.off()` produces a valid SVG
//! file containing the expected elements.

#![cfg(feature = "svg-device")]

use r::Session;

/// Helper: create a temp dir, run R code that uses absolute paths, return the dir.
fn run_svg_test(filename: &str, code: &str) -> (std::path::PathBuf, String) {
    let mut s = Session::new_with_captured_output();
    let dir = std::env::temp_dir().join(format!(
        "minir-svg-test-{}-{}",
        std::process::id(),
        filename.replace('.', "_")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let full_path = dir.join(filename);
    let path_str = full_path.to_string_lossy().to_string();

    // Replace {PATH} placeholder in code with the actual absolute path
    let code = code.replace("{PATH}", &path_str);
    s.eval_source(&code).unwrap();

    let content = if full_path.exists() {
        std::fs::read_to_string(&full_path).unwrap()
    } else {
        String::new()
    };

    (dir, content)
}

#[test]
fn svg_plot_points_creates_file() {
    let (dir, content) = run_svg_test(
        "test_points.svg",
        r#"
        svg("{PATH}")
        plot(1:10)
        dev.off()
    "#,
    );

    assert!(
        !content.is_empty(),
        "SVG file should be created and non-empty"
    );
    assert!(content.contains("<svg"), "Should contain SVG opening tag");
    assert!(content.contains("</svg>"), "Should contain SVG closing tag");
    assert!(
        content.contains("<circle"),
        "Should contain circle elements for points"
    );
    // 10 points for 1:10
    let circle_count = content.matches("<circle").count();
    assert_eq!(circle_count, 10, "Should have 10 circles for plot(1:10)");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_plot_with_title_and_labels() {
    let (dir, content) = run_svg_test(
        "test_labels.svg",
        r#"
        svg("{PATH}")
        plot(c(1, 2, 3), c(4, 5, 6), main = "My Title", xlab = "X Axis", ylab = "Y Axis")
        dev.off()
    "#,
    );

    assert!(content.contains("My Title"), "Should contain the title");
    assert!(content.contains("X Axis"), "Should contain x-axis label");
    assert!(content.contains("Y Axis"), "Should contain y-axis label");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_plot_lines_type() {
    let (dir, content) = run_svg_test(
        "test_lines.svg",
        r#"
        svg("{PATH}")
        plot(1:5, c(2, 4, 6, 8, 10), type = "l")
        dev.off()
    "#,
    );

    assert!(
        content.contains("<polyline"),
        "Should contain polyline for type='l'"
    );
    assert!(
        !content.contains("<circle"),
        "type='l' should not have circles"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_custom_dimensions() {
    let (dir, content) = run_svg_test(
        "test_dims.svg",
        r#"
        svg("{PATH}", width = 10, height = 5)
        plot(1:3)
        dev.off()
    "#,
    );

    // 10 inches * 96 DPI = 960 pixels wide
    assert!(content.contains("960"), "Width should be 960 pixels");
    // 5 inches * 96 DPI = 480 pixels tall
    assert!(content.contains("480"), "Height should be 480 pixels");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_plot_with_abline() {
    let (dir, content) = run_svg_test(
        "test_abline.svg",
        r#"
        svg("{PATH}")
        plot(1:5, 1:5)
        abline(h = 3, col = "red")
        abline(v = 2, col = "blue")
        dev.off()
    "#,
    );

    assert!(
        content.contains("<line"),
        "abline should produce line elements in SVG"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_dev_off_returns_one() {
    let mut s = Session::new_with_captured_output();
    let dir = std::env::temp_dir().join(format!("minir-svg-devoff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let svg_path = dir.join("devoff.svg");

    s.eval_source(&format!("svg('{}')", svg_path.to_string_lossy()))
        .unwrap();
    let result = s.eval_source("dev.off()").unwrap();

    // dev.off() should return integer 1
    match &result.value {
        r::interpreter::value::RValue::Vector(rv) => {
            assert_eq!(rv.inner.as_integer_scalar(), Some(1));
        }
        other => panic!("Expected integer vector, got {:?}", other),
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_barplot_creates_rects() {
    let (dir, content) = run_svg_test(
        "test_barplot.svg",
        r#"
        svg("{PATH}")
        barplot(c(3, 7, 2, 5))
        dev.off()
    "#,
    );

    // barplot should create rect elements (bars) plus the background and border rects
    // We expect at least 4 bar rects + 2 rects (background + border) = 6 total
    let rect_count = content.matches("<rect").count();
    assert!(
        rect_count >= 6,
        "Should have at least 6 rect elements (4 bars + background + border), got {rect_count}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_multiple_plots_last_wins() {
    let (dir, content) = run_svg_test(
        "test_last.svg",
        r#"
        svg("{PATH}")
        plot(1:3)
        plot(1:5)
        dev.off()
    "#,
    );

    // The second plot(1:5) should replace the first, so 5 circles
    let circle_count = content.matches("<circle").count();
    assert_eq!(
        circle_count, 5,
        "Second plot() should replace first; expected 5 circles, got {circle_count}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn png_device_creates_file() {
    let dir = std::env::temp_dir().join(format!("minir-png-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png_path = dir.join("test.png");

    let mut s = Session::new();
    s.eval_source(&format!(
        "png('{}'); plot(1:3); dev.off()",
        png_path.to_string_lossy()
    ))
    .unwrap();

    let svg_path = dir.join("test.svg");
    assert!(svg_path.exists(), "png() + dev.off() should create a file");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(feature = "pdf-device")]
fn pdf_device_creates_file() {
    let dir = std::env::temp_dir().join(format!("minir-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("test.pdf");

    let mut s = Session::new();
    s.eval_source(&format!(
        "pdf('{}'); plot(1:3); dev.off()",
        pdf_path.to_string_lossy()
    ))
    .unwrap();

    assert!(
        pdf_path.exists(),
        "pdf() + dev.off() should create a PDF file"
    );
    let content = std::fs::read(&pdf_path).unwrap();
    assert!(
        content.starts_with(b"%PDF"),
        "PDF file should start with %PDF header"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_axes_have_ticks() {
    let (dir, content) = run_svg_test(
        "test_axes.svg",
        r#"
        svg("{PATH}")
        plot(1:10)
        dev.off()
    "#,
    );

    // Should have tick lines on the axes
    let line_count = content.matches("<line").count();
    assert!(
        line_count >= 4,
        "Should have tick mark lines, got {line_count}"
    );

    // Should have tick labels (text elements beyond just axis labels)
    let text_count = content.matches("<text").count();
    assert!(
        text_count >= 4,
        "Should have tick label text elements, got {text_count}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_contains_valid_viewbox() {
    let (dir, content) = run_svg_test(
        "test_viewbox.svg",
        r#"
        svg("{PATH}")
        plot(1:3)
        dev.off()
    "#,
    );

    // Default 7x7 inches at 96 DPI = 672x672
    assert!(
        content.contains("viewBox"),
        "SVG should have a viewBox attribute"
    );
    assert!(content.contains("672"), "Default size should be 672 pixels");

    std::fs::remove_dir_all(&dir).ok();
}
