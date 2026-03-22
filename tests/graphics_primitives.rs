//! Tests for graphics drawing primitives and coordinate transforms.
//!
//! Since NullDevice discards all output, these tests verify that:
//! 1. Calling drawing primitives with valid args does not error
//! 2. Invalid arguments produce appropriate errors
//! 3. Coordinate transforms round-trip correctly
//! 4. pretty_ticks produces reasonable tick positions

use r::Session;

// region: Coordinate transform unit tests (via the public module)

#[test]
fn coord_transform_round_trip() {
    use r::interpreter::graphics::coord::CoordTransform;

    let ct = CoordTransform::new([0.0, 100.0, -50.0, 50.0], [0.0, 800.0, 0.0, 600.0]);

    // X round-trip
    for &x in &[0.0, 25.0, 50.0, 75.0, 100.0] {
        let dev_x = ct.usr_to_dev_x(x);
        let back = ct.dev_to_usr_x(dev_x);
        assert!(
            (back - x).abs() < 1e-10,
            "X round-trip failed: {x} -> {dev_x} -> {back}"
        );
    }

    // Y round-trip
    for &y in &[-50.0, -25.0, 0.0, 25.0, 50.0] {
        let dev_y = ct.usr_to_dev_y(y);
        let back = ct.dev_to_usr_y(dev_y);
        assert!(
            (back - y).abs() < 1e-10,
            "Y round-trip failed: {y} -> {dev_y} -> {back}"
        );
    }
}

#[test]
fn coord_transform_y_flip() {
    use r::interpreter::graphics::coord::CoordTransform;

    let ct = CoordTransform::new([0.0, 10.0, 0.0, 10.0], [0.0, 500.0, 0.0, 400.0]);

    // User y=0 (min) should map to device y=400 (max) due to Y-flip
    assert!((ct.usr_to_dev_y(0.0) - 400.0).abs() < 1e-10);
    // User y=10 (max) should map to device y=0 (min)
    assert!((ct.usr_to_dev_y(10.0) - 0.0).abs() < 1e-10);
}

#[test]
fn pretty_ticks_basic() {
    use r::interpreter::graphics::coord::pretty_ticks;

    let ticks = pretty_ticks(0.0, 10.0, 5);
    assert!(
        !ticks.is_empty(),
        "pretty_ticks should return at least one tick"
    );
    assert!(
        *ticks.first().unwrap() <= 0.0 + 1e-10,
        "first tick should be <= min"
    );
    assert!(
        *ticks.last().unwrap() >= 10.0 - 1e-10,
        "last tick should be >= max"
    );
    // Should have approximately 5-7 ticks for [0, 10]
    assert!(ticks.len() >= 3, "too few ticks: {ticks:?}");
    assert!(ticks.len() <= 15, "too many ticks: {ticks:?}");
}

#[test]
fn pretty_ticks_are_monotonic() {
    use r::interpreter::graphics::coord::pretty_ticks;

    for &(lo, hi, n) in &[
        (0.0, 10.0, 5),
        (-100.0, 100.0, 10),
        (0.0, 1.0, 5),
        (0.0, 1_000_000.0, 5),
    ] {
        let ticks = pretty_ticks(lo, hi, n);
        for i in 1..ticks.len() {
            assert!(
                ticks[i] >= ticks[i - 1],
                "ticks not monotonic for [{lo}, {hi}]: {ticks:?}"
            );
        }
    }
}

#[test]
fn pretty_ticks_includes_zero_for_symmetric_range() {
    use r::interpreter::graphics::coord::pretty_ticks;

    let ticks = pretty_ticks(-50.0, 50.0, 5);
    assert!(
        ticks.iter().any(|&t| t.abs() < 1e-10),
        "symmetric range should include 0 as a tick: {ticks:?}"
    );
}

// endregion

// region: Drawing primitive builtins (R-level tests)

#[test]
fn plot_new_does_not_error() {
    let mut s = Session::new();
    s.eval_source("plot.new()").unwrap();
}

#[test]
fn plot_window_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(-5, 5))
        "#,
    )
    .unwrap();
}

#[test]
fn plot_window_requires_xlim_and_ylim() {
    let mut s = Session::new();
    // Should error if xlim has length < 2
    let result = s.eval_source("plot.window(xlim = 1, ylim = c(0, 1))");
    assert!(result.is_err(), "plot.window with scalar xlim should error");
}

#[test]
fn points_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        points(c(1, 2, 3), c(4, 5, 6))
        "#,
    )
    .unwrap();
}

#[test]
fn points_with_named_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        points(c(1, 2, 3), c(4, 5, 6), pch = 16, col = "red", cex = 2)
        "#,
    )
    .unwrap();
}

#[test]
fn lines_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        lines(c(1, 5, 10), c(2, 8, 3))
        "#,
    )
    .unwrap();
}

#[test]
fn lines_with_named_args() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        lines(c(1, 5, 10), c(2, 8, 3), col = "blue", lwd = 2)
        "#,
    )
    .unwrap();
}

#[test]
fn segments_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        segments(c(1, 2), c(3, 4), c(5, 6), c(7, 8))
        "#,
    )
    .unwrap();
}

#[test]
fn rect_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        rect(1, 2, 5, 8)
        "#,
    )
    .unwrap();
}

#[test]
fn rect_with_fill_and_border() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        rect(1, 2, 5, 8, col = "lightblue", border = "darkblue")
        "#,
    )
    .unwrap();
}

#[test]
fn polygon_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        polygon(c(1, 5, 3), c(1, 1, 5))
        "#,
    )
    .unwrap();
}

#[test]
fn polygon_with_fill() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        polygon(c(1, 5, 3), c(1, 1, 5), col = "green", border = "black")
        "#,
    )
    .unwrap();
}

#[test]
fn text_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        text(5, 5, "hello")
        "#,
    )
    .unwrap();
}

#[test]
fn text_with_multiple_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        text(c(2, 5, 8), c(3, 6, 9), c("a", "b", "c"))
        "#,
    )
    .unwrap();
}

#[test]
fn abline_horizontal() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        abline(h = 5)
        "#,
    )
    .unwrap();
}

#[test]
fn abline_vertical() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        abline(v = c(2, 4, 6, 8))
        "#,
    )
    .unwrap();
}

#[test]
fn abline_slope_intercept() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        abline(0, 1)
        "#,
    )
    .unwrap();
}

#[test]
fn title_does_not_error() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        title(main = "My Plot", xlab = "X Axis", ylab = "Y Axis")
        "#,
    )
    .unwrap();
}

#[test]
fn title_with_subtitle() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        title("Main Title", sub = "Subtitle")
        "#,
    )
    .unwrap();
}

#[test]
fn axis_bottom() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 100), ylim = c(0, 100))
        axis(1)
        "#,
    )
    .unwrap();
}

#[test]
fn axis_left() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 100), ylim = c(0, 100))
        axis(2)
        "#,
    )
    .unwrap();
}

#[test]
fn axis_with_custom_ticks() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        axis(1, at = c(0, 2.5, 5, 7.5, 10))
        "#,
    )
    .unwrap();
}

#[test]
fn axis_with_custom_labels() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        axis(1, at = c(0, 5, 10), labels = c("low", "mid", "high"))
        "#,
    )
    .unwrap();
}

#[test]
fn axis_invalid_side_errors() {
    let mut s = Session::new();
    let result = s.eval_source(
        r#"
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 10))
        axis(5)
        "#,
    );
    assert!(result.is_err(), "axis(5) should error on invalid side");
}

#[test]
fn dev_cur_returns_one() {
    let mut s = Session::new();
    s.eval_source("stopifnot(dev.cur() == 1L)").unwrap();
}

#[test]
fn dev_off_returns_one() {
    let mut s = Session::new();
    s.eval_source("stopifnot(dev.off() == 1L)").unwrap();
}

#[test]
fn par_returns_list() {
    let mut s = Session::new();
    s.eval_source("stopifnot(is.list(par()))").unwrap();
}

#[test]
fn full_plot_workflow() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # A complete plotting workflow that should run without errors
        plot.new()
        plot.window(xlim = c(0, 10), ylim = c(0, 20))

        # Draw data
        points(c(1, 3, 5, 7, 9), c(2, 8, 12, 15, 18))
        lines(c(0, 10), c(0, 20), col = "red")

        # Add reference lines
        abline(h = 10)
        abline(v = 5)

        # Add annotations
        text(5, 10, "Center")
        rect(2, 5, 8, 15, col = "lightgray")

        # Add axes and title
        axis(1)
        axis(2)
        title(main = "Test Plot", xlab = "X", ylab = "Y")
        "#,
    )
    .unwrap();
}

#[test]
fn legend_does_not_error() {
    let mut s = Session::new();
    s.eval_source(r#"legend("topright", legend = c("a", "b"), col = c("red", "blue"))"#)
        .unwrap();
}

// endregion
