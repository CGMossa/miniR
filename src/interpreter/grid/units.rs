//! Grid unit system — flexible measurements for grid graphics.
//!
//! R's grid package uses a rich unit system where lengths can be expressed in
//! physical units (cm, inches, points), relative units (npc, snpc), data
//! coordinates (native), or text-metric units (lines, char, strwidth).
//! Units can be combined with arithmetic (`+`, `-`, `*`) to create compound
//! measurements that are resolved at drawing time.

use std::ops::{Add, Sub};

// region: UnitType

/// The type of measurement for a grid unit value.
#[derive(Clone, Debug, PartialEq)]
pub enum UnitType {
    /// Normalized parent coordinates (0-1 maps to parent viewport extent).
    Npc,
    /// Centimeters.
    Cm,
    /// Inches.
    Inches,
    /// Millimeters.
    Mm,
    /// Points (1/72 inch, typographic).
    Points,
    /// Text line heights (fontsize * lineheight).
    Lines,
    /// Character widths (approximate, based on fontsize).
    Char,
    /// Data coordinates (mapped through viewport xscale/yscale).
    Native,
    /// Flexible/proportional space (resolved by layout engine, returns 0 before layout).
    Null,
    /// Square normalized parent coordinates (uses the smaller of width/height).
    Snpc,
    /// Width of the given string in the current font.
    StrWidth(String),
    /// Height of the given string in the current font.
    StrHeight(String),
    /// Width of a named grob.
    GrobWidth(String),
    /// Height of a named grob.
    GrobHeight(String),
}

impl UnitType {
    /// Whether this unit type is absolute (resolution does not depend on viewport size).
    pub fn is_absolute(&self) -> bool {
        matches!(
            self,
            UnitType::Cm | UnitType::Inches | UnitType::Mm | UnitType::Points
        )
    }
}

// endregion

// region: UnitContext

/// Context needed to resolve relative and data-dependent units to centimeters.
///
/// This captures the current viewport dimensions and font metrics at
/// drawing time so that relative units (npc, native, lines, etc.) can be
/// converted to physical measurements.
#[derive(Clone, Debug)]
pub struct UnitContext {
    /// Viewport width in centimeters.
    pub viewport_width_cm: f64,
    /// Viewport height in centimeters.
    pub viewport_height_cm: f64,
    /// Data coordinate range on the x-axis: (min, max).
    pub xscale: (f64, f64),
    /// Data coordinate range on the y-axis: (min, max).
    pub yscale: (f64, f64),
    /// Font size in points.
    pub fontsize_pt: f64,
    /// Line height multiplier (e.g. 1.2).
    pub lineheight: f64,
}

impl Default for UnitContext {
    fn default() -> Self {
        UnitContext {
            viewport_width_cm: 17.78,  // ~7 inches (default R device width)
            viewport_height_cm: 17.78, // ~7 inches
            xscale: (0.0, 1.0),
            yscale: (0.0, 1.0),
            fontsize_pt: 12.0,
            lineheight: 1.2,
        }
    }
}

// endregion

// region: Axis

/// Which axis a unit is being resolved along. Matters for relative units
/// like `Npc` (which uses width for x, height for y) and `Native`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

// endregion

// region: Unit

/// A grid unit — one or more (value, type) pairs representing a measurement.
///
/// Like R's `unit()`, a `Unit` can contain multiple values with different
/// types. Arithmetic on units produces compound units: `unit(1, "cm") + unit(2, "mm")`
/// becomes a single `Unit` with two entries that are summed at resolution time.
#[derive(Clone, Debug)]
pub struct Unit {
    /// The numeric values.
    pub values: Vec<f64>,
    /// The unit type for each value (parallel to `values`).
    pub units: Vec<UnitType>,
}

impl Unit {
    /// Create a unit with a single value and type.
    pub fn new(value: f64, unit_type: UnitType) -> Self {
        Unit {
            values: vec![value],
            units: vec![unit_type],
        }
    }

    /// Shorthand: normalized parent coordinates.
    pub fn npc(value: f64) -> Self {
        Unit::new(value, UnitType::Npc)
    }

    /// Shorthand: centimeters.
    pub fn cm(value: f64) -> Self {
        Unit::new(value, UnitType::Cm)
    }

    /// Shorthand: inches.
    pub fn inches(value: f64) -> Self {
        Unit::new(value, UnitType::Inches)
    }

    /// Shorthand: millimeters.
    pub fn mm(value: f64) -> Self {
        Unit::new(value, UnitType::Mm)
    }

    /// Shorthand: points (1/72 inch).
    pub fn points(value: f64) -> Self {
        Unit::new(value, UnitType::Points)
    }

    /// Shorthand: text line heights.
    pub fn lines(value: f64) -> Self {
        Unit::new(value, UnitType::Lines)
    }

    /// Shorthand: null (flexible/proportional) unit.
    pub fn null(value: f64) -> Self {
        Unit::new(value, UnitType::Null)
    }

    /// Number of (value, type) pairs in this unit.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether this unit contains no values.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether all component unit types are absolute (no viewport/data dependency).
    pub fn is_absolute(&self) -> bool {
        self.units.iter().all(UnitType::is_absolute)
    }

    /// Resolve all values to centimeters along the given axis.
    ///
    /// For a compound unit (created by `+` or `-`), the result is a single
    /// value: the sum of all component values converted to cm.
    /// For a simple unit, returns a one-element vector.
    pub fn to_cm(&self, ctx: &UnitContext, axis: Axis) -> Vec<f64> {
        if self.values.is_empty() {
            return vec![];
        }

        // Compound units (from Add/Sub) are summed into a single resolved value.
        // Simple units return one value per entry.
        let resolved: Vec<f64> = self
            .values
            .iter()
            .zip(self.units.iter())
            .map(|(&val, unit_type)| resolve_one(val, unit_type, ctx, axis))
            .collect();

        resolved
    }

    /// Resolve to a single cm value by summing all components.
    ///
    /// This is the most common usage: a compound unit like `unit(1, "cm") + unit(5, "mm")`
    /// resolves to `1.5` cm.
    pub fn to_cm_scalar(&self, ctx: &UnitContext, axis: Axis) -> f64 {
        self.values
            .iter()
            .zip(self.units.iter())
            .map(|(&val, unit_type)| resolve_one(val, unit_type, ctx, axis))
            .sum()
    }

    /// Scalar multiply: scale all values by a factor.
    pub fn scale(mut self, factor: f64) -> Self {
        for v in &mut self.values {
            *v *= factor;
        }
        self
    }
}

/// Resolve a single (value, unit_type) pair to centimeters.
fn resolve_one(value: f64, unit_type: &UnitType, ctx: &UnitContext, axis: Axis) -> f64 {
    let viewport_cm = match axis {
        Axis::X => ctx.viewport_width_cm,
        Axis::Y => ctx.viewport_height_cm,
    };

    match unit_type {
        UnitType::Npc => value * viewport_cm,
        UnitType::Cm => value,
        UnitType::Inches => value * 2.54,
        UnitType::Mm => value / 10.0,
        UnitType::Points => value / 72.0 * 2.54,
        UnitType::Lines => {
            // One line = fontsize_pt * lineheight, converted from points to cm
            value * ctx.fontsize_pt * ctx.lineheight / 72.0 * 2.54
        }
        UnitType::Char => {
            // Approximate character width as 0.6 * fontsize in points, to cm
            value * ctx.fontsize_pt * 0.6 / 72.0 * 2.54
        }
        UnitType::Native => {
            // Map data coordinate to npc, then to cm
            let (scale_min, scale_max) = match axis {
                Axis::X => ctx.xscale,
                Axis::Y => ctx.yscale,
            };
            let range = scale_max - scale_min;
            if range.abs() < f64::EPSILON {
                0.0
            } else {
                let npc = (value - scale_min) / range;
                npc * viewport_cm
            }
        }
        UnitType::Null => {
            // Null units are resolved by the layout engine; before layout, they are 0.
            0.0
        }
        UnitType::Snpc => {
            // Square NPC: use the smaller of width and height
            let min_dim = ctx.viewport_width_cm.min(ctx.viewport_height_cm);
            value * min_dim
        }
        UnitType::StrWidth(s) => {
            // Rough estimate: each character is ~0.6 * fontsize in points
            let char_count = s.len() as f64;
            char_count * ctx.fontsize_pt * 0.6 / 72.0 * 2.54
        }
        UnitType::StrHeight(_) => {
            // Rough estimate: string height is ~1.0 * fontsize in points
            ctx.fontsize_pt / 72.0 * 2.54
        }
        UnitType::GrobWidth(_) | UnitType::GrobHeight(_) => {
            // Grob dimensions require looking up the grob by name and measuring it.
            // This is not yet implemented; return 0 as a stub.
            0.0
        }
    }
}

impl Add for Unit {
    type Output = Unit;

    fn add(mut self, mut rhs: Unit) -> Unit {
        self.values.append(&mut rhs.values);
        self.units.append(&mut rhs.units);
        self
    }
}

impl Sub for Unit {
    type Output = Unit;

    fn sub(mut self, mut rhs: Unit) -> Unit {
        // Negate all RHS values so that summing the compound unit produces a - b
        for v in &mut rhs.values {
            *v = -*v;
        }
        self.values.append(&mut rhs.values);
        self.units.append(&mut rhs.units);
        self
    }
}

// endregion

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn unit_cm_passthrough() {
        let u = Unit::cm(2.5);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 2.5), "expected 2.5, got {result}");
    }

    #[test]
    fn unit_inches_conversion() {
        let u = Unit::inches(1.0);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 2.54), "expected 2.54, got {result}");
    }

    #[test]
    fn unit_mm_conversion() {
        let u = Unit::mm(10.0);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 1.0), "expected 1.0, got {result}");
    }

    #[test]
    fn unit_points_conversion() {
        // 72 points = 1 inch = 2.54 cm
        let u = Unit::points(72.0);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 2.54), "expected 2.54, got {result}");
    }

    #[test]
    fn unit_npc_uses_viewport_dimension() {
        let u = Unit::npc(0.5);
        let ctx = UnitContext {
            viewport_width_cm: 20.0,
            viewport_height_cm: 10.0,
            ..Default::default()
        };
        // X axis: 0.5 * 20 = 10
        let x = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(x, 10.0), "expected 10.0, got {x}");
        // Y axis: 0.5 * 10 = 5
        let y = u.to_cm_scalar(&ctx, Axis::Y);
        assert!(approx_eq(y, 5.0), "expected 5.0, got {y}");
    }

    #[test]
    fn unit_addition_combines_values() {
        let a = Unit::cm(1.0);
        let b = Unit::mm(5.0);
        let combined = a + b;
        assert_eq!(combined.len(), 2);
        let ctx = UnitContext::default();
        let result = combined.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 1.5), "expected 1.5, got {result}");
    }

    #[test]
    fn unit_subtraction() {
        let a = Unit::cm(3.0);
        let b = Unit::cm(1.0);
        let combined = a - b;
        let ctx = UnitContext::default();
        let result = combined.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 2.0), "expected 2.0, got {result}");
    }

    #[test]
    fn unit_scale() {
        let u = Unit::cm(2.0).scale(3.0);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 6.0), "expected 6.0, got {result}");
    }

    #[test]
    fn unit_null_resolves_to_zero() {
        let u = Unit::null(5.0);
        let ctx = UnitContext::default();
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 0.0), "expected 0.0, got {result}");
    }

    #[test]
    fn unit_native_maps_through_scale() {
        let u = Unit::new(50.0, UnitType::Native);
        let ctx = UnitContext {
            viewport_width_cm: 10.0,
            xscale: (0.0, 100.0),
            ..Default::default()
        };
        // 50 in [0, 100] -> npc 0.5 -> 0.5 * 10 = 5 cm
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 5.0), "expected 5.0, got {result}");
    }

    #[test]
    fn unit_is_absolute() {
        assert!(Unit::cm(1.0).is_absolute());
        assert!(Unit::inches(1.0).is_absolute());
        assert!(Unit::mm(1.0).is_absolute());
        assert!(Unit::points(1.0).is_absolute());
        assert!(!Unit::npc(0.5).is_absolute());
        assert!(!Unit::null(1.0).is_absolute());
        assert!(!Unit::lines(1.0).is_absolute());
    }

    #[test]
    fn unit_snpc_uses_smaller_dimension() {
        let u = Unit::new(1.0, UnitType::Snpc);
        let ctx = UnitContext {
            viewport_width_cm: 20.0,
            viewport_height_cm: 10.0,
            ..Default::default()
        };
        // snpc uses min(20, 10) = 10
        let result = u.to_cm_scalar(&ctx, Axis::X);
        assert!(approx_eq(result, 10.0), "expected 10.0, got {result}");
    }

    #[test]
    fn unit_empty() {
        let u = Unit {
            values: vec![],
            units: vec![],
        };
        assert!(u.is_empty());
        assert_eq!(u.to_cm(&UnitContext::default(), Axis::X).len(), 0);
    }
}
