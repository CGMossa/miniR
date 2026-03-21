//! Tests for f64 formatting, particularly with the `fast-format` (zmij) feature.

use r::Session;

// region: format_r_double output correctness

#[test]
fn format_double_integer_valued() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Integer-valued doubles should print without decimal point
        stopifnot(identical(format(1), "1"))
        stopifnot(identical(format(42), "42"))
        stopifnot(identical(format(-7), "-7"))
        stopifnot(identical(format(0), "0"))
        stopifnot(identical(format(1e14), "100000000000000"))
        "#,
    )
    .unwrap();
}

#[test]
fn format_double_fractional() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Fractional doubles should keep their decimal representation
        stopifnot(identical(format(1.5), "1.5"))
        stopifnot(identical(format(0.1), "0.1"))
        stopifnot(identical(format(3.125), "3.125"))
        stopifnot(identical(format(-2.5), "-2.5"))
        "#,
    )
    .unwrap();
}

#[test]
fn format_double_special_values() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # NaN, Inf, -Inf
        stopifnot(identical(format(NaN), "NaN"))
        stopifnot(identical(format(Inf), "Inf"))
        stopifnot(identical(format(-Inf), "-Inf"))
        "#,
    )
    .unwrap();
}

#[test]
fn format_double_scientific_notation() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # Very small and very large numbers use scientific notation
        x <- 6.62607015e-34
        formatted <- format(x)
        # Should contain 'e' for scientific notation
        stopifnot(grepl("e", formatted))
        stopifnot(grepl("6.62607015", formatted))
        "#,
    )
    .unwrap();
}

// endregion

// region: paste/print roundtrip

#[test]
fn paste_preserves_double_format() {
    let mut s = Session::new();
    s.eval_source(
        r#"
        # paste() should use the same formatting as print/format
        stopifnot(identical(paste(1.5), "1.5"))
        stopifnot(identical(paste(0.1), "0.1"))
        stopifnot(identical(paste(Inf), "Inf"))
        stopifnot(identical(paste(NaN), "NaN"))
        "#,
    )
    .unwrap();
}

// endregion

// region: unit tests for format_r_double

#[test]
fn format_r_double_unit_tests() {
    use r::interpreter::value::format_r_double;

    // Integer-valued doubles
    assert_eq!(format_r_double(0.0), "0");
    assert_eq!(format_r_double(1.0), "1");
    assert_eq!(format_r_double(-1.0), "-1");
    assert_eq!(format_r_double(42.0), "42");
    assert_eq!(format_r_double(1000000.0), "1000000");

    // Fractional values
    assert_eq!(format_r_double(1.5), "1.5");
    assert_eq!(format_r_double(0.1), "0.1");
    assert_eq!(format_r_double(3.125), "3.125");
    assert_eq!(format_r_double(-2.5), "-2.5");

    // Special values
    assert_eq!(format_r_double(f64::NAN), "NaN");
    assert_eq!(format_r_double(f64::INFINITY), "Inf");
    assert_eq!(format_r_double(f64::NEG_INFINITY), "-Inf");

    // Negative zero should format as "0" (not "-0")
    assert_eq!(format_r_double(-0.0), "0");

    // Large integer-valued doubles still format as integers
    assert_eq!(format_r_double(999999999999999.0), "999999999999999");

    // Scientific notation for very small numbers
    let small = format_r_double(1e-20);
    assert!(
        small.contains("e-"),
        "Expected scientific notation for 1e-20, got: {small}"
    );

    // Scientific notation for very large non-integer doubles
    let large = format_r_double(1.23e20);
    // This is a non-integer large double, should use some representation
    assert!(!large.is_empty());
}

// endregion
