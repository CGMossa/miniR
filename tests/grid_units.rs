//! Integration tests for grid graphics unit system and graphical parameters.

use r::interpreter::graphics::par::{FontFace, LineType};
use r::interpreter::grid::gpar::{Gpar, LineEnd, LineJoin};
use r::interpreter::grid::units::{Axis, Unit, UnitContext, UnitType};

const EPSILON: f64 = 1e-9;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

// region: Unit creation and basic properties

#[test]
fn unit_new_creates_single_value() {
    let u = Unit::new(5.0, UnitType::Cm);
    assert_eq!(u.len(), 1);
    assert!(!u.is_empty());
    assert!(u.is_absolute());
}

#[test]
fn unit_shorthands() {
    let cm = Unit::cm(1.0);
    assert_eq!(cm.units[0], UnitType::Cm);

    let inches = Unit::inches(1.0);
    assert_eq!(inches.units[0], UnitType::Inches);

    let npc = Unit::npc(0.5);
    assert_eq!(npc.units[0], UnitType::Npc);

    let mm = Unit::mm(10.0);
    assert_eq!(mm.units[0], UnitType::Mm);

    let pts = Unit::points(12.0);
    assert_eq!(pts.units[0], UnitType::Points);

    let lines = Unit::lines(2.0);
    assert_eq!(lines.units[0], UnitType::Lines);

    let null = Unit::null(1.0);
    assert_eq!(null.units[0], UnitType::Null);
}

#[test]
fn unit_absolute_detection() {
    assert!(Unit::cm(1.0).is_absolute());
    assert!(Unit::inches(1.0).is_absolute());
    assert!(Unit::mm(1.0).is_absolute());
    assert!(Unit::points(12.0).is_absolute());

    assert!(!Unit::npc(0.5).is_absolute());
    assert!(!Unit::null(1.0).is_absolute());
    assert!(!Unit::lines(1.0).is_absolute());
    assert!(!Unit::new(1.0, UnitType::Native).is_absolute());
    assert!(!Unit::new(1.0, UnitType::Snpc).is_absolute());
    assert!(!Unit::new(1.0, UnitType::Char).is_absolute());
}

// endregion

// region: Unit resolution to cm

#[test]
fn resolve_cm_passthrough() {
    let ctx = UnitContext::default();
    let result = Unit::cm(3.17).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 3.17), "expected 3.17, got {result}");
}

#[test]
fn resolve_inches_to_cm() {
    let ctx = UnitContext::default();
    let result = Unit::inches(2.0).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 5.08), "expected 5.08, got {result}");
}

#[test]
fn resolve_mm_to_cm() {
    let ctx = UnitContext::default();
    let result = Unit::mm(25.4).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 2.54), "expected 2.54, got {result}");
}

#[test]
fn resolve_points_to_cm() {
    let ctx = UnitContext::default();
    // 36 points = 0.5 inch = 1.27 cm
    let result = Unit::points(36.0).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 1.27), "expected 1.27, got {result}");
}

#[test]
fn resolve_npc_x_and_y() {
    let ctx = UnitContext {
        viewport_width_cm: 20.0,
        viewport_height_cm: 15.0,
        ..Default::default()
    };

    let u = Unit::npc(0.25);
    let x = u.to_cm_scalar(&ctx, Axis::X);
    let y = u.to_cm_scalar(&ctx, Axis::Y);

    assert!(approx_eq(x, 5.0), "X: expected 5.0, got {x}");
    assert!(approx_eq(y, 3.75), "Y: expected 3.75, got {y}");
}

#[test]
fn resolve_native_through_scale() {
    let ctx = UnitContext {
        viewport_width_cm: 10.0,
        viewport_height_cm: 8.0,
        xscale: (100.0, 200.0),
        yscale: (0.0, 50.0),
        ..Default::default()
    };

    // x=150 in [100, 200] -> npc 0.5 -> 5.0 cm
    let x_result = Unit::new(150.0, UnitType::Native).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(x_result, 5.0), "expected 5.0, got {x_result}");

    // y=25 in [0, 50] -> npc 0.5 -> 4.0 cm
    let y_result = Unit::new(25.0, UnitType::Native).to_cm_scalar(&ctx, Axis::Y);
    assert!(approx_eq(y_result, 4.0), "expected 4.0, got {y_result}");
}

#[test]
fn resolve_lines_uses_font_metrics() {
    let ctx = UnitContext {
        fontsize_pt: 10.0,
        lineheight: 1.5,
        ..Default::default()
    };
    // 1 line = 10 * 1.5 / 72 * 2.54 = 15 / 72 * 2.54
    let expected = 10.0 * 1.5 / 72.0 * 2.54;
    let result = Unit::lines(1.0).to_cm_scalar(&ctx, Axis::X);
    assert!(
        approx_eq(result, expected),
        "expected {expected}, got {result}"
    );
}

#[test]
fn resolve_snpc_uses_smaller_dimension() {
    let ctx = UnitContext {
        viewport_width_cm: 30.0,
        viewport_height_cm: 10.0,
        ..Default::default()
    };
    // snpc uses min(30, 10) = 10
    let result = Unit::new(0.5, UnitType::Snpc).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 5.0), "expected 5.0, got {result}");
}

#[test]
fn resolve_null_is_zero() {
    let ctx = UnitContext::default();
    let result = Unit::null(42.0).to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 0.0), "expected 0.0, got {result}");
}

#[test]
fn resolve_strwidth_estimate() {
    let ctx = UnitContext {
        fontsize_pt: 12.0,
        ..Default::default()
    };
    let u = Unit::new(1.0, UnitType::StrWidth("hello".to_string()));
    // 5 chars * 12 * 0.6 / 72 * 2.54
    let expected = 5.0 * 12.0 * 0.6 / 72.0 * 2.54;
    let result = u.to_cm_scalar(&ctx, Axis::X);
    assert!(
        approx_eq(result, expected),
        "expected {expected}, got {result}"
    );
}

// endregion

// region: Unit arithmetic

#[test]
fn unit_add_combines_components() {
    let a = Unit::cm(2.0);
    let b = Unit::inches(1.0);
    let combined = a + b;

    assert_eq!(combined.len(), 2);
    let ctx = UnitContext::default();
    let result = combined.to_cm_scalar(&ctx, Axis::X);
    // 2.0 + 2.54 = 4.54
    assert!(approx_eq(result, 4.54), "expected 4.54, got {result}");
}

#[test]
fn unit_sub_negates_rhs() {
    let a = Unit::cm(5.0);
    let b = Unit::mm(20.0); // 2.0 cm
    let combined = a - b;

    let ctx = UnitContext::default();
    let result = combined.to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 3.0), "expected 3.0, got {result}");
}

#[test]
fn unit_scale_multiplies_all_values() {
    let u = Unit::cm(2.0);
    let scaled = u.scale(3.0);

    let ctx = UnitContext::default();
    let result = scaled.to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 6.0), "expected 6.0, got {result}");
}

#[test]
fn unit_chain_arithmetic() {
    // 1 cm + 0.5 inches - 5 mm = 1.0 + 1.27 - 0.5 = 1.77
    let u = Unit::cm(1.0) + Unit::inches(0.5) - Unit::mm(5.0);

    let ctx = UnitContext::default();
    let result = u.to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 1.77), "expected 1.77, got {result}");
}

#[test]
fn unit_mixed_absolute_and_relative() {
    let u = Unit::cm(1.0) + Unit::npc(0.5);
    assert!(!u.is_absolute());

    let ctx = UnitContext {
        viewport_width_cm: 10.0,
        ..Default::default()
    };
    // 1.0 + 0.5 * 10 = 6.0
    let result = u.to_cm_scalar(&ctx, Axis::X);
    assert!(approx_eq(result, 6.0), "expected 6.0, got {result}");
}

#[test]
fn unit_to_cm_returns_per_component() {
    let u = Unit::cm(1.0) + Unit::cm(2.0);
    let ctx = UnitContext::default();
    let components = u.to_cm(&ctx, Axis::X);
    assert_eq!(components.len(), 2);
    assert!(approx_eq(components[0], 1.0));
    assert!(approx_eq(components[1], 2.0));
}

// endregion

// region: Gpar

#[test]
fn gpar_default_effective_values() {
    let g = Gpar::new();
    assert_eq!(g.effective_col(), [0, 0, 0, 255]); // black
    assert_eq!(g.effective_fill(), [255, 255, 255, 0]); // transparent
    assert!(approx_eq(g.effective_lwd(), 1.0));
    assert!(approx_eq(g.effective_fontsize(), 12.0));
    assert!(approx_eq(g.effective_lineheight(), 1.2));
    assert!(approx_eq(g.effective_cex(), 1.0));
    assert!(approx_eq(g.effective_alpha(), 1.0));
    assert_eq!(g.effective_font(), FontFace::Plain);
    assert_eq!(g.effective_fontfamily(), "sans");
    assert_eq!(g.effective_lty(), LineType::Solid);
    assert_eq!(g.effective_lineend(), LineEnd::Round);
    assert_eq!(g.effective_linejoin(), LineJoin::Round);
    assert!(approx_eq(g.effective_linemitre(), 10.0));
}

#[test]
fn gpar_inheritance_basic() {
    let parent = Gpar {
        col: Some([255, 0, 0, 255]),
        fontsize: Some(18.0),
        lwd: Some(2.0),
        fontfamily: Some("mono".to_string()),
        lineend: Some(LineEnd::Butt),
        ..Default::default()
    };

    let mut child = Gpar {
        fontsize: Some(10.0),
        ..Default::default()
    };

    child.inherit_from(&parent);

    // Inherited
    assert_eq!(child.col, Some([255, 0, 0, 255]));
    assert_eq!(child.lwd, Some(2.0));
    assert_eq!(child.fontfamily, Some("mono".to_string()));
    assert_eq!(child.lineend, Some(LineEnd::Butt));

    // Child override preserved
    assert_eq!(child.fontsize, Some(10.0));

    // Not set on either
    assert!(child.fill.is_none());
}

#[test]
fn gpar_with_parent_immutable() {
    let parent = Gpar {
        col: Some([0, 128, 0, 255]),
        lty: Some(LineType::Dashed),
        ..Default::default()
    };

    let child = Gpar {
        fill: Some([255, 255, 0, 200]),
        ..Default::default()
    };

    let resolved = child.with_parent(&parent);

    // Resolved has both
    assert_eq!(resolved.col, Some([0, 128, 0, 255]));
    assert_eq!(resolved.fill, Some([255, 255, 0, 200]));
    assert_eq!(resolved.lty, Some(LineType::Dashed));

    // Original child unchanged
    assert!(child.col.is_none());
    assert!(child.lty.is_none());
}

#[test]
fn gpar_multi_level_inheritance() {
    let grandparent = Gpar {
        col: Some([255, 0, 0, 255]),
        fontsize: Some(20.0),
        lwd: Some(3.0),
        ..Default::default()
    };

    let parent = Gpar {
        fontsize: Some(14.0), // overrides grandparent
        ..Default::default()
    };

    let child = Gpar {
        lwd: Some(1.0), // overrides grandparent
        ..Default::default()
    };

    // Resolve parent from grandparent, then child from resolved parent
    let resolved_parent = parent.with_parent(&grandparent);
    let resolved_child = child.with_parent(&resolved_parent);

    assert_eq!(resolved_child.col, Some([255, 0, 0, 255])); // from grandparent
    assert_eq!(resolved_child.fontsize, Some(14.0)); // from parent
    assert_eq!(resolved_child.lwd, Some(1.0)); // from child
}

// endregion
