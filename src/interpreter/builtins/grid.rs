//! Grid graphics builtins — R-facing functions for the grid graphics system.
//!
//! Grid objects are represented as R lists with S3 class attributes:
//! - Unit: `list(value=..., units=...)` with class "unit"
//! - Gpar: `list(col=..., fill=..., ...)` with class "gpar"
//! - Viewport: `list(x=..., y=..., width=..., ...)` with class "viewport"
//! - Grobs: `list(x=..., y=..., gp=..., ...)` with class c("<type>", "grob")

use super::CallArgs;
use crate::interpreter::grid;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

// region: Helpers

/// Create an RValue::List with the given named entries and set its class attribute.
fn make_grid_object(entries: Vec<(String, RValue)>, classes: &[&str]) -> RValue {
    let values: Vec<(Option<String>, RValue)> =
        entries.into_iter().map(|(k, v)| (Some(k), v)).collect();
    let mut list = RList::new(values);
    let class_vec: Vec<Option<String>> = classes.iter().map(|c| Some(c.to_string())).collect();
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(class_vec.into())),
    );
    RValue::List(list)
}

/// Extract an optional RValue from args by name or position, returning NULL if absent.
fn opt_value(args: &CallArgs, name: &str, pos: usize) -> RValue {
    args.value(name, pos).cloned().unwrap_or(RValue::Null)
}

/// Normalize the `just` parameter: convert string names like "centre", "left",
/// "top" to numeric c(hjust, vjust) pairs matching R's grid convention.
fn normalize_just(ca: &CallArgs, name: &str, pos: usize) -> RValue {
    let val = ca.value(name, pos).cloned().unwrap_or(RValue::Null);
    if let RValue::Vector(ref rv) = val {
        if let Some(s) = rv.inner.as_character_scalar() {
            let (h, v) = match s.as_str() {
                "left" => (0.0, 0.5),
                "right" => (1.0, 0.5),
                "top" => (0.5, 1.0),
                "bottom" => (0.5, 0.0),
                "centre" | "center" => (0.5, 0.5),
                "bottom.left" | "bottomleft" => (0.0, 0.0),
                "bottom.right" | "bottomright" => (1.0, 0.0),
                "top.left" | "topleft" => (0.0, 1.0),
                "top.right" | "topright" => (1.0, 1.0),
                _ => return val, // unknown string, pass through
            };
            return RValue::vec(Vector::Double(vec![Some(h), Some(v)].into()));
        }
    }
    val
}

/// Generate a unique grob name with the given prefix and a counter.
fn auto_grob_name(prefix: &str, ctx: &BuiltinContext) -> String {
    // Use the display list length as a simple counter for unique names
    let n = ctx.interpreter().grid_display_list.borrow().len();
    format!("{prefix}.{n}")
}

/// Record a grob on the grid display list.
fn record_on_display_list(grob: &RValue, ctx: &BuiltinContext) {
    ctx.interpreter()
        .grid_display_list
        .borrow_mut()
        .push(grob.clone());
}

/// Wrap a unit value: if x is already a unit object, return it; otherwise
/// create `unit(x, default_units)`.
fn ensure_unit(value: &RValue, default_units: &str) -> RValue {
    // Check if it's already a unit (list with class "unit")
    if let RValue::List(list) = value {
        if let Some(classes) = list.class() {
            if classes.iter().any(|c| c == "unit") {
                return value.clone();
            }
        }
    }
    // Otherwise wrap in a unit
    make_grid_object(
        vec![
            ("value".to_string(), value.clone()),
            (
                "units".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some(default_units.to_string())].into(),
                )),
            ),
        ],
        &["unit"],
    )
}

/// Default NPC unit value (0.5 npc).
fn default_npc(val: f64) -> RValue {
    make_grid_object(
        vec![
            (
                "value".to_string(),
                RValue::vec(Vector::Double(vec![Some(val)].into())),
            ),
            (
                "units".to_string(),
                RValue::vec(Vector::Character(vec![Some("npc".to_string())].into())),
            ),
        ],
        &["unit"],
    )
}

/// Default unit of 0 npc.
fn default_npc_zero() -> RValue {
    default_npc(0.0)
}

/// Default unit of 1 npc for width/height.
fn default_npc_one() -> RValue {
    default_npc(1.0)
}

/// Default unit of 0.5 npc for x/y.
fn default_npc_half() -> RValue {
    default_npc(0.5)
}

/// Find a viewport by name in the viewport stack.
/// Returns the index in the stack if found.
fn find_viewport_by_name(name: &str, ctx: &BuiltinContext) -> Option<usize> {
    let stack = ctx.interpreter().grid_viewport_stack.borrow();
    for (i, vp) in stack.iter().enumerate() {
        if let RValue::List(list) = vp {
            for (key, val) in &list.values {
                if key.as_deref() == Some("name") {
                    if let RValue::Vector(rv) = val {
                        if rv.inner.as_character_scalar().as_deref() == Some(name) {
                            return Some(i);
                        }
                    }
                }
            }
        }
    }
    None
}

// endregion

// region: R-to-Rust conversion helpers

/// Extract a Rust `Unit` from an R value representing a unit object.
///
/// Handles both:
/// - A unit object (list with class "unit", fields "value" and "units")
/// - A bare numeric vector (treated as NPC by default)
fn extract_unit_from_rvalue(val: &RValue) -> grid::units::Unit {
    if let RValue::List(list) = val {
        // Look for "value" and "units" entries
        let mut values_opt = None;
        let mut units_opt = None;
        for (key, v) in &list.values {
            match key.as_deref() {
                Some("value") => values_opt = Some(v),
                Some("units") => units_opt = Some(v),
                _ => {}
            }
        }

        if let (Some(values_val), Some(units_val)) = (values_opt, units_opt) {
            let nums: Vec<f64> = if let Some(rv) = values_val.as_vector() {
                rv.to_doubles()
                    .into_iter()
                    .map(|v| v.unwrap_or(0.0))
                    .collect()
            } else {
                vec![0.0]
            };

            let unit_strs: Vec<String> = if let Some(rv) = units_val.as_vector() {
                rv.to_characters()
                    .into_iter()
                    .map(|v: Option<String>| v.unwrap_or_else(|| "npc".to_string()))
                    .collect()
            } else {
                vec!["npc".to_string()]
            };

            let mut unit_types = Vec::with_capacity(unit_strs.len());
            for s in &unit_strs {
                unit_types.push(parse_unit_type(s));
            }

            // Recycle to match lengths
            let n = nums.len().max(unit_types.len());
            let values: Vec<f64> = (0..n).map(|i| nums[i % nums.len()]).collect();
            let units: Vec<grid::units::UnitType> = (0..n)
                .map(|i| unit_types[i % unit_types.len()].clone())
                .collect();

            return grid::units::Unit { values, units };
        }
    }

    // Bare numeric — treat as NPC
    if let Some(rv) = val.as_vector() {
        let nums: Vec<f64> = rv
            .to_doubles()
            .into_iter()
            .map(|v| v.unwrap_or(0.0))
            .collect();
        if !nums.is_empty() {
            return grid::units::Unit {
                values: nums.clone(),
                units: nums.iter().map(|_| grid::units::UnitType::Npc).collect(),
            };
        }
    }

    // Default: 0.5 npc
    grid::units::Unit::npc(0.5)
}

/// Parse a unit type string into a `UnitType`.
fn parse_unit_type(s: &str) -> grid::units::UnitType {
    match s {
        "npc" => grid::units::UnitType::Npc,
        "cm" => grid::units::UnitType::Cm,
        "inches" | "in" => grid::units::UnitType::Inches,
        "mm" => grid::units::UnitType::Mm,
        "points" | "pt" | "bigpts" | "picas" | "dida" | "cicero" | "scaledpts" => {
            grid::units::UnitType::Points
        }
        "lines" => grid::units::UnitType::Lines,
        "char" => grid::units::UnitType::Char,
        "native" => grid::units::UnitType::Native,
        "null" => grid::units::UnitType::Null,
        "snpc" => grid::units::UnitType::Snpc,
        "strwidth" => grid::units::UnitType::StrWidth(String::new()),
        "strheight" => grid::units::UnitType::StrHeight(String::new()),
        "grobwidth" => grid::units::UnitType::GrobWidth(String::new()),
        "grobheight" => grid::units::UnitType::GrobHeight(String::new()),
        _ => grid::units::UnitType::Npc,
    }
}

/// Parse an R color value to RGBA.
///
/// Supports: color name strings, hex strings (#RRGGBB / #RRGGBBAA),
/// and integer indices (treated as palette lookups, default black).
fn parse_grid_color(value: &RValue) -> Option<[u8; 4]> {
    match value {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                if let Some(Some(s)) = c.first() {
                    Some(parse_color_string(s))
                } else {
                    None
                }
            }
            Vector::Integer(iv) => {
                // Basic palette: 0=white, 1=black, 2=red, 3=green, 4=blue, ...
                iv.first_opt().map(|i| match i {
                    0 => [255, 255, 255, 0],   // transparent
                    1 => [0, 0, 0, 255],       // black
                    2 => [255, 0, 0, 255],     // red
                    3 => [0, 128, 0, 255],     // green
                    4 => [0, 0, 255, 255],     // blue
                    5 => [0, 255, 255, 255],   // cyan
                    6 => [255, 0, 255, 255],   // magenta
                    7 => [255, 255, 0, 255],   // yellow
                    8 => [128, 128, 128, 255], // gray
                    _ => [0, 0, 0, 255],       // default black
                })
            }
            _ => None,
        },
        RValue::Null => None,
        _ => None,
    }
}

/// Parse a hex color string to RGBA.
fn parse_color_string(s: &str) -> [u8; 4] {
    // Named colors
    match s.to_lowercase().as_str() {
        "black" => return [0, 0, 0, 255],
        "white" => return [255, 255, 255, 255],
        "red" => return [255, 0, 0, 255],
        "green" => return [0, 128, 0, 255],
        "blue" => return [0, 0, 255, 255],
        "cyan" => return [0, 255, 255, 255],
        "magenta" => return [255, 0, 255, 255],
        "yellow" => return [255, 255, 0, 255],
        "gray" | "grey" => return [190, 190, 190, 255],
        "orange" => return [255, 165, 0, 255],
        "purple" => return [128, 0, 128, 255],
        "brown" => return [165, 42, 42, 255],
        "pink" => return [255, 192, 203, 255],
        "transparent" | "na" => return [0, 0, 0, 0],
        _ => {}
    }

    // Hex format
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                return [r, g, b, 255];
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
                return [r, g, b, a];
            }
            _ => {}
        }
    }

    [0, 0, 0, 255] // default black
}

/// Extract a Rust `Gpar` from an R gpar list object.
fn extract_gpar_from_rvalue(val: &RValue) -> grid::gpar::Gpar {
    let mut gp = grid::gpar::Gpar::new();

    if let RValue::List(list) = val {
        for (key, v) in &list.values {
            match key.as_deref() {
                Some("col") => {
                    gp.col = parse_grid_color(v);
                }
                Some("fill") => {
                    gp.fill = parse_grid_color(v);
                }
                Some("lwd") => {
                    if let Some(rv) = v.as_vector() {
                        gp.lwd = rv.as_double_scalar();
                    }
                }
                Some("fontsize") => {
                    if let Some(rv) = v.as_vector() {
                        gp.fontsize = rv.as_double_scalar();
                    }
                }
                Some("lineheight") => {
                    if let Some(rv) = v.as_vector() {
                        gp.lineheight = rv.as_double_scalar();
                    }
                }
                Some("cex") => {
                    if let Some(rv) = v.as_vector() {
                        gp.cex = rv.as_double_scalar();
                    }
                }
                Some("alpha") => {
                    if let Some(rv) = v.as_vector() {
                        gp.alpha = rv.as_double_scalar();
                    }
                }
                _ => {}
            }
        }
    }

    gp
}

/// Extract justification from an R value.
///
/// Accepts: a character string ("centre", "left", etc.), a numeric vector
/// c(hjust, vjust), or NULL (returns default centre/centre).
fn extract_justification(
    val: &RValue,
) -> (grid::viewport::Justification, grid::viewport::Justification) {
    use grid::viewport::Justification;

    match val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(c) => {
                if c.len() >= 2 {
                    let h = c[0]
                        .as_deref()
                        .and_then(Justification::parse)
                        .unwrap_or(Justification::Centre);
                    let v_j = c[1]
                        .as_deref()
                        .and_then(Justification::parse)
                        .unwrap_or(Justification::Centre);
                    (h, v_j)
                } else if let Some(Some(s)) = c.first() {
                    let j = Justification::parse(s).unwrap_or(Justification::Centre);
                    (j, j)
                } else {
                    (Justification::Centre, Justification::Centre)
                }
            }
            Vector::Double(d) => {
                let h = d.first_opt().unwrap_or(0.5);
                let v_val = d.get_opt(1).unwrap_or(h);
                (num_to_just(h), num_to_just(v_val))
            }
            _ => (Justification::Centre, Justification::Centre),
        },
        RValue::Null => (Justification::Centre, Justification::Centre),
        _ => (Justification::Centre, Justification::Centre),
    }
}

/// Convert a numeric justification (0.0, 0.5, 1.0) to a `Justification`.
fn num_to_just(v: f64) -> grid::viewport::Justification {
    use grid::viewport::Justification;
    if v <= 0.25 {
        Justification::Left
    } else if v >= 0.75 {
        Justification::Right
    } else {
        Justification::Centre
    }
}

/// Extract a Rust `Viewport` from an R viewport list object.
fn extract_viewport_from_rvalue(val: &RValue) -> grid::viewport::Viewport {
    let mut vp = grid::viewport::Viewport::new();

    if let RValue::List(list) = val {
        for (key, v) in &list.values {
            match key.as_deref() {
                Some("x") => vp.x = extract_unit_from_rvalue(v),
                Some("y") => vp.y = extract_unit_from_rvalue(v),
                Some("width") => vp.width = extract_unit_from_rvalue(v),
                Some("height") => vp.height = extract_unit_from_rvalue(v),
                Some("just") => {
                    let (h, v_just) = extract_justification(v);
                    vp.just = (h, v_just);
                }
                Some("xscale") => {
                    if let Some(rv) = v.as_vector() {
                        let doubles = rv.to_doubles();
                        if doubles.len() >= 2 {
                            vp.xscale = (doubles[0].unwrap_or(0.0), doubles[1].unwrap_or(1.0));
                        }
                    }
                }
                Some("yscale") => {
                    if let Some(rv) = v.as_vector() {
                        let doubles = rv.to_doubles();
                        if doubles.len() >= 2 {
                            vp.yscale = (doubles[0].unwrap_or(0.0), doubles[1].unwrap_or(1.0));
                        }
                    }
                }
                Some("angle") => {
                    if let Some(rv) = v.as_vector() {
                        vp.angle = rv.as_double_scalar().unwrap_or(0.0);
                    }
                }
                Some("clip") => {
                    if let Some(rv) = v.as_vector() {
                        if let Some(s) = rv.as_character_scalar() {
                            vp.clip = s == "on";
                        }
                    }
                }
                Some("gp") => {
                    vp.gp = extract_gpar_from_rvalue(v);
                }
                Some("name") => {
                    if let Some(rv) = v.as_vector() {
                        vp.name = rv.as_character_scalar();
                    }
                }
                _ => {}
            }
        }
    }

    vp
}

/// Record a Rust grob on the Rust display list.
///
/// Creates the Rust `Grob`, adds it to the `GrobStore`, and records a
/// `DisplayItem::Draw` on the Rust `DisplayList`.
fn record_rust_grob(grob: grid::grob::Grob, ctx: &BuiltinContext) {
    let grob_id = ctx.interpreter().grid_grob_store.borrow_mut().add(grob);
    ctx.interpreter()
        .grid_rust_display_list
        .borrow_mut()
        .record(grid::display::DisplayItem::Draw(grob_id));
}

/// Extract a vector of label strings from an R value.
fn extract_labels(val: &RValue) -> Vec<String> {
    match val {
        RValue::Vector(rv) => rv
            .inner
            .to_characters()
            .into_iter()
            .map(|v| v.unwrap_or_default())
            .collect(),
        _ => vec![],
    }
}

/// Extract rotation angle from an R value.
fn extract_rot(val: &RValue) -> f64 {
    if let Some(rv) = val.as_vector() {
        rv.as_double_scalar().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Extract pch (plotting character) from an R value.
fn extract_pch(val: &RValue) -> u8 {
    if let Some(rv) = val.as_vector() {
        rv.as_integer_scalar()
            .and_then(|i| u8::try_from(i).ok())
            .unwrap_or(1)
    } else {
        1
    }
}

// endregion

// region: Page management

/// Clear the grid display list and viewport stack, starting a new page.
///
/// @return NULL (invisibly)
#[interpreter_builtin(name = "grid.newpage", namespace = "grid")]
fn interp_grid_newpage(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Flush any existing grid content before clearing
    flush_grid_to_plot(context);

    // Clear R-level state
    context.interpreter().grid_display_list.borrow_mut().clear();
    context
        .interpreter()
        .grid_viewport_stack
        .borrow_mut()
        .clear();

    // Clear Rust-level state
    context
        .interpreter()
        .grid_rust_display_list
        .borrow_mut()
        .clear();
    *context.interpreter().grid_grob_store.borrow_mut() = grid::grob::GrobStore::new();
    *context.interpreter().grid_rust_viewport_stack.borrow_mut() =
        grid::viewport::ViewportStack::new(17.78, 17.78);

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Record a grob on the grid display list for later rendering.
///
/// @param grob a grob object to draw
/// @return the grob (invisibly)
#[interpreter_builtin(name = "grid.draw", namespace = "grid", min_args = 1)]
fn interp_grid_draw(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let grob = args[0].clone();
    record_on_display_list(&grob, context);

    // Also record on the Rust display list if we can determine the grob type
    if let RValue::List(list) = &grob {
        if let Some(classes) = list.class() {
            // The first class before "grob" is the type class
            let type_class = classes
                .iter()
                .find(|c| *c != "grob")
                .cloned()
                .unwrap_or_default();

            // Build entries from the list
            let entries: Vec<(String, RValue)> = list
                .values
                .iter()
                .filter_map(|(k, v)| k.as_ref().map(|k| (k.clone(), v.clone())))
                .collect();

            if let Some(rust_grob) = build_rust_grob(&type_class, &entries) {
                record_rust_grob(rust_grob, context);
            }
        }
    }

    context.interpreter().set_invisible();
    Ok(grob)
}

// endregion

// region: Unit constructor

/// Create a unit object representing a measurement with given units.
///
/// Supported units include: "npc", "cm", "inches", "mm", "points", "lines",
/// "native", "null", "char", "grobwidth", "grobheight", "strwidth", "strheight".
///
/// @param x numeric value(s) for the unit
/// @param units character string specifying the unit type
/// @param data optional data for special units (e.g., grob for "grobwidth")
/// @return a unit object (list with class "unit")
#[interpreter_builtin(namespace = "grid", min_args = 2)]
fn interp_unit(
    args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let x = ca.value("x", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "unit() requires an 'x' argument".to_string(),
        )
    })?;
    let units_str = ca.string("units", 1)?;
    let data = opt_value(&ca, "data", 2);

    // Validate units
    let valid_units = [
        "npc",
        "cm",
        "inches",
        "mm",
        "points",
        "lines",
        "native",
        "null",
        "char",
        "grobwidth",
        "grobheight",
        "strwidth",
        "strheight",
        "picas",
        "bigpts",
        "dida",
        "cicero",
        "scaledpts",
    ];
    if !valid_units.contains(&units_str.as_str()) {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "invalid unit '{}'. Valid units are: {}",
                units_str,
                valid_units.join(", ")
            ),
        ));
    }

    // Vectorize: if x is a vector, replicate the units string to match
    let units_val = if let Some(rv) = x.as_vector() {
        let n = rv.len();
        let units_vec: Vec<Option<String>> = (0..n).map(|_| Some(units_str.clone())).collect();
        RValue::vec(Vector::Character(units_vec.into()))
    } else {
        RValue::vec(Vector::Character(vec![Some(units_str.clone())].into()))
    };

    let mut entries = vec![
        ("value".to_string(), x.clone()),
        ("units".to_string(), units_val),
    ];
    if !matches!(data, RValue::Null) {
        entries.push(("data".to_string(), data));
    }

    Ok(make_grid_object(entries, &["unit"]))
}

// endregion

// region: Gpar constructor

/// Create a graphical parameter object (gpar) for grid graphics.
///
/// @param col line color
/// @param fill fill color
/// @param lwd line width
/// @param lty line type
/// @param fontsize font size in points
/// @param font font face (1=plain, 2=bold, 3=italic, 4=bold-italic)
/// @param fontfamily font family name
/// @param lineheight line height multiplier
/// @param cex character expansion factor
/// @param alpha alpha transparency
/// @return a gpar object (list with class "gpar")
#[interpreter_builtin(namespace = "grid")]
fn interp_gpar(
    _args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mut entries: Vec<(String, RValue)> = Vec::new();

    let param_names = [
        "col",
        "fill",
        "lwd",
        "lty",
        "fontsize",
        "font",
        "fontfamily",
        "lineheight",
        "cex",
        "alpha",
        "lex",
        "lineend",
        "linejoin",
        "linemitre",
    ];

    for &name in &param_names {
        if let Some((_, val)) = named.iter().find(|(k, _)| k == name) {
            entries.push((name.to_string(), val.clone()));
        }
    }

    Ok(make_grid_object(entries, &["gpar"]))
}

// endregion

// region: Viewport functions

/// Create a viewport object for grid graphics.
///
/// @param x horizontal position (default: unit(0.5, "npc"))
/// @param y vertical position (default: unit(0.5, "npc"))
/// @param width viewport width (default: unit(1, "npc"))
/// @param height viewport height (default: unit(1, "npc"))
/// @param just justification ("centre", "left", "right", "top", "bottom")
/// @param xscale numeric c(min, max) for x-axis scale
/// @param yscale numeric c(min, max) for y-axis scale
/// @param angle rotation angle in degrees
/// @param clip clipping ("on", "off", "inherit")
/// @param gp graphical parameters (gpar object)
/// @param layout a grid layout
/// @param name viewport name for navigation
/// @return a viewport object (list with class "viewport")
#[interpreter_builtin(namespace = "grid")]
fn interp_viewport(
    _args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let get = |name: &str| -> Option<RValue> {
        named
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };

    let x = get("x").unwrap_or_else(default_npc_half);
    let y = get("y").unwrap_or_else(default_npc_half);
    let width = get("width").unwrap_or_else(default_npc_one);
    let height = get("height").unwrap_or_else(default_npc_one);
    let just = get("just")
        .unwrap_or_else(|| RValue::vec(Vector::Character(vec![Some("centre".to_string())].into())));
    let xscale = get("xscale")
        .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into())));
    let yscale = get("yscale")
        .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into())));
    let angle = get("angle").unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0)].into())));
    let clip = get("clip").unwrap_or_else(|| {
        RValue::vec(Vector::Character(vec![Some("inherit".to_string())].into()))
    });
    let gp = get("gp").unwrap_or(RValue::Null);
    let layout = get("layout").unwrap_or(RValue::Null);
    let name = get("name").unwrap_or(RValue::Null);
    let layout_pos_row = get("layout.pos.row").unwrap_or(RValue::Null);
    let layout_pos_col = get("layout.pos.col").unwrap_or(RValue::Null);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("width".to_string(), width),
        ("height".to_string(), height),
        ("just".to_string(), just),
        ("xscale".to_string(), xscale),
        ("yscale".to_string(), yscale),
        ("angle".to_string(), angle),
        ("clip".to_string(), clip),
        ("gp".to_string(), gp),
        ("layout".to_string(), layout),
        ("name".to_string(), name),
        ("layout.pos.row".to_string(), layout_pos_row),
        ("layout.pos.col".to_string(), layout_pos_col),
    ];

    Ok(make_grid_object(entries, &["viewport"]))
}

/// Push a viewport onto the viewport stack.
///
/// @param vp a viewport object
/// @return NULL (invisibly)
#[interpreter_builtin(name = "pushViewport", namespace = "grid", min_args = 1)]
fn interp_push_viewport(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let vp = args[0].clone();

    // Record the push on the R-level display list
    let push_record = make_grid_object(
        vec![
            (
                "type".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("pushViewport".to_string())].into(),
                )),
            ),
            ("viewport".to_string(), vp.clone()),
        ],
        &["vpOperation"],
    );
    record_on_display_list(&push_record, context);

    // Also push on the Rust-level viewport stack and display list
    let rust_vp = extract_viewport_from_rvalue(&vp);
    context
        .interpreter()
        .grid_rust_viewport_stack
        .borrow_mut()
        .push(rust_vp.clone());
    context
        .interpreter()
        .grid_rust_display_list
        .borrow_mut()
        .record(grid::display::DisplayItem::PushViewport(Box::new(rust_vp)));

    context
        .interpreter()
        .grid_viewport_stack
        .borrow_mut()
        .push(vp);
    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Pop viewports from the viewport stack.
///
/// @param n number of viewports to pop (default 1)
/// @return NULL (invisibly)
#[interpreter_builtin(name = "popViewport", namespace = "grid")]
fn interp_pop_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = ca.integer_or("n", 0, 1);
    let n = usize::try_from(n).unwrap_or(0);

    let mut stack = context.interpreter().grid_viewport_stack.borrow_mut();
    for _ in 0..n {
        if stack.is_empty() {
            return Err(RError::new(
                RErrorKind::Other,
                "cannot pop the top-level viewport".to_string(),
            ));
        }
        stack.pop();
    }

    // Also pop from the Rust-level viewport stack and record on Rust display list
    drop(stack);
    {
        let mut rust_stack = context.interpreter().grid_rust_viewport_stack.borrow_mut();
        let mut rust_dl = context.interpreter().grid_rust_display_list.borrow_mut();
        for _ in 0..n {
            rust_stack.pop();
            rust_dl.record(grid::display::DisplayItem::PopViewport);
        }
    }

    // Record the pop on the R-level display list
    let pop_record = make_grid_object(
        vec![
            (
                "type".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("popViewport".to_string())].into(),
                )),
            ),
            (
                "n".to_string(),
                RValue::vec(Vector::Integer(vec![Some(i64::from(n as i32))].into())),
            ),
        ],
        &["vpOperation"],
    );
    record_on_display_list(&pop_record, context);

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Return the current (topmost) viewport.
///
/// @return the current viewport object, or a default root viewport
#[interpreter_builtin(name = "current.viewport", namespace = "grid")]
fn interp_current_viewport(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let stack = context.interpreter().grid_viewport_stack.borrow();
    match stack.last() {
        Some(vp) => Ok(vp.clone()),
        None => {
            // Return a default root viewport
            Ok(make_grid_object(
                vec![
                    ("x".to_string(), default_npc_half()),
                    ("y".to_string(), default_npc_half()),
                    ("width".to_string(), default_npc_one()),
                    ("height".to_string(), default_npc_one()),
                    (
                        "name".to_string(),
                        RValue::vec(Vector::Character(vec![Some("ROOT".to_string())].into())),
                    ),
                ],
                &["viewport"],
            ))
        }
    }
}

/// Navigate up the viewport stack without popping.
///
/// @param n number of levels to navigate up (default 1)
/// @return NULL (invisibly)
#[interpreter_builtin(name = "upViewport", namespace = "grid")]
fn interp_up_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let n = ca.integer_or("n", 0, 1);
    let n = usize::try_from(n).unwrap_or(0);

    let stack_len = context.interpreter().grid_viewport_stack.borrow().len();
    if n > stack_len {
        return Err(RError::new(
            RErrorKind::Other,
            format!("cannot navigate up {n} viewport(s) — only {stack_len} on the stack"),
        ));
    }

    // upViewport navigates without popping — in our simplified model,
    // we record the operation but keep the stack intact for query purposes.
    let up_record = make_grid_object(
        vec![
            (
                "type".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("upViewport".to_string())].into(),
                )),
            ),
            (
                "n".to_string(),
                RValue::vec(Vector::Integer(vec![Some(i64::from(n as i32))].into())),
            ),
        ],
        &["vpOperation"],
    );
    record_on_display_list(&up_record, context);

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Navigate down to a named viewport in the stack.
///
/// @param name the name of the viewport to navigate to
/// @return the depth navigated (integer), or error if not found
#[interpreter_builtin(name = "downViewport", namespace = "grid", min_args = 1)]
fn interp_down_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let name = ca.string("name", 0)?;

    match find_viewport_by_name(&name, context) {
        Some(idx) => {
            let stack_len = context.interpreter().grid_viewport_stack.borrow().len();
            let depth = stack_len.saturating_sub(idx + 1);
            Ok(RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(depth).unwrap_or(0))].into(),
            )))
        }
        None => Err(RError::new(
            RErrorKind::Other,
            format!("viewport '{name}' was not found"),
        )),
    }
}

/// Find and navigate to a named viewport anywhere in the viewport tree.
///
/// @param name the name of the viewport to seek
/// @return the depth navigated (integer), or error if not found
#[interpreter_builtin(name = "seekViewport", namespace = "grid", min_args = 1)]
fn interp_seek_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let name = ca.string("name", 0)?;

    match find_viewport_by_name(&name, context) {
        Some(idx) => {
            let stack_len = context.interpreter().grid_viewport_stack.borrow().len();
            let depth = stack_len.saturating_sub(idx + 1);
            Ok(RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(depth).unwrap_or(0))].into(),
            )))
        }
        None => Err(RError::new(
            RErrorKind::Other,
            format!("viewport '{name}' was not found"),
        )),
    }
}

/// Create a viewport with margins specified in lines of text.
///
/// This is a convenience wrapper around viewport() that converts margin
/// specifications (in lines of text) to appropriate offsets.
///
/// @param margins numeric vector c(bottom, left, top, right) in lines (default c(5.1, 4.1, 4.1, 2.1))
/// @return a viewport object
#[interpreter_builtin(name = "plotViewport", namespace = "grid")]
fn interp_plot_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let margins = if let Some(val) = ca.value("margins", 0) {
        if let Some(rv) = val.as_vector() {
            rv.to_doubles()
        } else {
            vec![Some(5.1), Some(4.1), Some(4.1), Some(2.1)]
        }
    } else {
        vec![Some(5.1), Some(4.1), Some(4.1), Some(2.1)]
    };

    // margins: c(bottom, left, top, right)
    let bottom = margins.first().copied().flatten().unwrap_or(5.1);
    let left = margins.get(1).copied().flatten().unwrap_or(4.1);
    let top = margins.get(2).copied().flatten().unwrap_or(4.1);
    let right = margins.get(3).copied().flatten().unwrap_or(2.1);

    // Create viewport with margin-adjusted position and size
    let entries = vec![
        (
            "x".to_string(),
            make_grid_object(
                vec![
                    (
                        "value".to_string(),
                        RValue::vec(Vector::Double(vec![Some(0.5 * (left - right))].into())),
                    ),
                    (
                        "units".to_string(),
                        RValue::vec(Vector::Character(vec![Some("lines".to_string())].into())),
                    ),
                ],
                &["unit"],
            ),
        ),
        (
            "y".to_string(),
            make_grid_object(
                vec![
                    (
                        "value".to_string(),
                        RValue::vec(Vector::Double(vec![Some(0.5 * (bottom - top))].into())),
                    ),
                    (
                        "units".to_string(),
                        RValue::vec(Vector::Character(vec![Some("lines".to_string())].into())),
                    ),
                ],
                &["unit"],
            ),
        ),
        (
            "width".to_string(),
            make_grid_object(
                vec![
                    (
                        "value".to_string(),
                        RValue::vec(Vector::Double(vec![Some(-(left + right))].into())),
                    ),
                    (
                        "units".to_string(),
                        RValue::vec(Vector::Character(vec![Some("lines".to_string())].into())),
                    ),
                ],
                &["unit"],
            ),
        ),
        (
            "height".to_string(),
            make_grid_object(
                vec![
                    (
                        "value".to_string(),
                        RValue::vec(Vector::Double(vec![Some(-(bottom + top))].into())),
                    ),
                    (
                        "units".to_string(),
                        RValue::vec(Vector::Character(vec![Some("lines".to_string())].into())),
                    ),
                ],
                &["unit"],
            ),
        ),
        (
            "just".to_string(),
            RValue::vec(Vector::Character(vec![Some("centre".to_string())].into())),
        ),
    ];

    Ok(make_grid_object(entries, &["viewport"]))
}

/// Create a viewport with scales determined by data ranges.
///
/// @param xData numeric vector of x-axis data
/// @param yData numeric vector of y-axis data
/// @param extension fraction to extend scales beyond data range (default 0.05)
/// @return a viewport object with appropriate xscale/yscale
#[interpreter_builtin(name = "dataViewport", namespace = "grid")]
fn interp_data_viewport(
    args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let extension = ca
        .value("extension", 2)
        .and_then(|v| v.as_vector()?.as_double_scalar())
        .unwrap_or(0.05);

    // Compute xscale from xData
    let xscale = if let Some(xdata) = ca.value("xData", 0) {
        if let Some(rv) = xdata.as_vector() {
            let doubles: Vec<f64> = rv.to_doubles().into_iter().flatten().collect();
            if doubles.is_empty() {
                vec![Some(0.0), Some(1.0)]
            } else {
                let min = doubles.iter().copied().fold(f64::INFINITY, f64::min);
                let max = doubles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let range = max - min;
                vec![Some(min - range * extension), Some(max + range * extension)]
            }
        } else {
            vec![Some(0.0), Some(1.0)]
        }
    } else {
        vec![Some(0.0), Some(1.0)]
    };

    // Compute yscale from yData
    let yscale = if let Some(ydata) = ca.value("yData", 1) {
        if let Some(rv) = ydata.as_vector() {
            let doubles: Vec<f64> = rv.to_doubles().into_iter().flatten().collect();
            if doubles.is_empty() {
                vec![Some(0.0), Some(1.0)]
            } else {
                let min = doubles.iter().copied().fold(f64::INFINITY, f64::min);
                let max = doubles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let range = max - min;
                vec![Some(min - range * extension), Some(max + range * extension)]
            }
        } else {
            vec![Some(0.0), Some(1.0)]
        }
    } else {
        vec![Some(0.0), Some(1.0)]
    };

    let entries = vec![
        ("x".to_string(), default_npc_half()),
        ("y".to_string(), default_npc_half()),
        ("width".to_string(), default_npc_one()),
        ("height".to_string(), default_npc_one()),
        (
            "just".to_string(),
            RValue::vec(Vector::Character(vec![Some("centre".to_string())].into())),
        ),
        (
            "xscale".to_string(),
            RValue::vec(Vector::Double(xscale.into())),
        ),
        (
            "yscale".to_string(),
            RValue::vec(Vector::Double(yscale.into())),
        ),
    ];

    Ok(make_grid_object(entries, &["viewport"]))
}

// endregion

// region: Grob primitives

/// Helper to create a grob and optionally draw it.
///
/// 1. Creates the grob R object (list with class `c(type_class, "grob")`)
/// 2. If `draw=TRUE`, records it on the display list
/// 3. If `vp` is provided, pushes the viewport, records the grob, pops the viewport
/// 4. Returns the grob invisibly
fn make_grob(
    type_class: &str,
    entries: Vec<(String, RValue)>,
    draw: bool,
    vp: RValue,
    ctx: &BuiltinContext,
) -> Result<RValue, RError> {
    let grob = make_grid_object(entries.clone(), &[type_class, "grob"]);

    if draw {
        // Build the Rust-level grob from the R entries
        let rust_grob = build_rust_grob(type_class, &entries);

        match vp {
            RValue::Null => {
                record_on_display_list(&grob, ctx);
                if let Some(rg) = rust_grob {
                    record_rust_grob(rg, ctx);
                }
            }
            _ => {
                // Push viewport, draw, pop — both R-level and Rust-level
                let push_record = make_grid_object(
                    vec![
                        (
                            "type".to_string(),
                            RValue::vec(Vector::Character(
                                vec![Some("pushViewport".to_string())].into(),
                            )),
                        ),
                        ("viewport".to_string(), vp.clone()),
                    ],
                    &["vpOperation"],
                );
                record_on_display_list(&push_record, ctx);

                // Rust-level viewport push
                let rust_vp = extract_viewport_from_rvalue(&vp);
                ctx.interpreter()
                    .grid_rust_viewport_stack
                    .borrow_mut()
                    .push(rust_vp.clone());
                ctx.interpreter()
                    .grid_rust_display_list
                    .borrow_mut()
                    .record(grid::display::DisplayItem::PushViewport(Box::new(rust_vp)));

                ctx.interpreter().grid_viewport_stack.borrow_mut().push(vp);

                record_on_display_list(&grob, ctx);
                if let Some(rg) = rust_grob {
                    record_rust_grob(rg, ctx);
                }

                ctx.interpreter().grid_viewport_stack.borrow_mut().pop();

                // Rust-level viewport pop
                ctx.interpreter()
                    .grid_rust_viewport_stack
                    .borrow_mut()
                    .pop();
                ctx.interpreter()
                    .grid_rust_display_list
                    .borrow_mut()
                    .record(grid::display::DisplayItem::PopViewport);

                let pop_record = make_grid_object(
                    vec![
                        (
                            "type".to_string(),
                            RValue::vec(Vector::Character(
                                vec![Some("popViewport".to_string())].into(),
                            )),
                        ),
                        (
                            "n".to_string(),
                            RValue::vec(Vector::Integer(vec![Some(1)].into())),
                        ),
                    ],
                    &["vpOperation"],
                );
                record_on_display_list(&pop_record, ctx);
            }
        }
    }

    ctx.interpreter().set_invisible();
    Ok(grob)
}

/// Build a Rust-level `Grob` from a type class name and list entries.
///
/// Returns `None` for grob types we don't yet have Rust-level support for
/// (e.g. axes, which are composite grobs).
fn build_rust_grob(type_class: &str, entries: &[(String, RValue)]) -> Option<grid::grob::Grob> {
    // Helper to look up an entry by name
    let get =
        |name: &str| -> Option<&RValue> { entries.iter().find(|(k, _)| k == name).map(|(_, v)| v) };

    match type_class {
        "lines" => {
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Lines { x, y, gp })
        }
        "segments" => {
            let x0 = extract_unit_from_rvalue(get("x0").unwrap_or(&RValue::Null));
            let y0 = extract_unit_from_rvalue(get("y0").unwrap_or(&RValue::Null));
            let x1 = extract_unit_from_rvalue(get("x1").unwrap_or(&RValue::Null));
            let y1 = extract_unit_from_rvalue(get("y1").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Segments { x0, y0, x1, y1, gp })
        }
        "points" => {
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let pch = extract_pch(get("pch").unwrap_or(&RValue::Null));
            let size = if let Some(sv) = get("size") {
                if matches!(sv, RValue::Null) {
                    grid::units::Unit::points(4.0)
                } else {
                    extract_unit_from_rvalue(sv)
                }
            } else {
                grid::units::Unit::points(4.0)
            };
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Points {
                x,
                y,
                pch,
                size,
                gp,
            })
        }
        "rect" => {
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let width = extract_unit_from_rvalue(get("width").unwrap_or(&RValue::Null));
            let height = extract_unit_from_rvalue(get("height").unwrap_or(&RValue::Null));
            let just = extract_justification(get("just").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Rect {
                x,
                y,
                width,
                height,
                just,
                gp,
            })
        }
        "circle" => {
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let r = extract_unit_from_rvalue(get("r").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Circle { x, y, r, gp })
        }
        "polygon" => {
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Polygon { x, y, gp })
        }
        "text" => {
            let label = extract_labels(get("label").unwrap_or(&RValue::Null));
            let x = extract_unit_from_rvalue(get("x").unwrap_or(&RValue::Null));
            let y = extract_unit_from_rvalue(get("y").unwrap_or(&RValue::Null));
            let just = extract_justification(get("just").unwrap_or(&RValue::Null));
            let rot = extract_rot(get("rot").unwrap_or(&RValue::Null));
            let gp = extract_gpar_from_rvalue(get("gp").unwrap_or(&RValue::Null));
            Some(grid::grob::Grob::Text {
                label,
                x,
                y,
                just,
                rot,
                gp,
            })
        }
        // Axes and other composite grobs don't have direct Rust Grob equivalents yet
        _ => None,
    }
}

/// Draw line segments (polyline) on the grid graphics device.
///
/// @param x x-coordinates
/// @param y y-coordinates
/// @param default.units default unit type for coordinates
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a lines grob (invisibly)
#[interpreter_builtin(name = "grid.lines", namespace = "grid")]
fn interp_grid_lines(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 2)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 6, true);
    let vp = opt_value(&ca, "vp", 4);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.lines", context));

    let x = ca
        .value("x", 0)
        .cloned()
        .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into())));
    let y = ca
        .value("y", 1)
        .cloned()
        .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into())));

    let x = ensure_unit(&x, &default_units);
    let y = ensure_unit(&y, &default_units);

    let gp = opt_value(&ca, "gp", 3);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("lines", entries, draw, vp, context)
}

/// Draw line segments between pairs of points.
///
/// @param x0 x-coordinates of start points
/// @param y0 y-coordinates of start points
/// @param x1 x-coordinates of end points
/// @param y1 y-coordinates of end points
/// @param default.units default unit type for coordinates
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a segments grob (invisibly)
#[interpreter_builtin(name = "grid.segments", namespace = "grid")]
fn interp_grid_segments(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 4)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 8, true);
    let vp = opt_value(&ca, "vp", 6);
    let name = ca
        .optional_string("name", 7)
        .unwrap_or_else(|| auto_grob_name("GRID.segments", context));

    let x0 = ensure_unit(
        &ca.value("x0", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y0 = ensure_unit(
        &ca.value("y0", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let x1 = ensure_unit(
        &ca.value("x1", 2).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y1 = ensure_unit(
        &ca.value("y1", 3).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let gp = opt_value(&ca, "gp", 5);

    let entries = vec![
        ("x0".to_string(), x0),
        ("y0".to_string(), y0),
        ("x1".to_string(), x1),
        ("y1".to_string(), y1),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("segments", entries, draw, vp, context)
}

/// Draw points on the grid graphics device.
///
/// @param x x-coordinates
/// @param y y-coordinates
/// @param pch plotting character (default 1)
/// @param size point size
/// @param default.units default unit type for coordinates
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a points grob (invisibly)
#[interpreter_builtin(name = "grid.points", namespace = "grid")]
fn interp_grid_points(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 4)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 8, true);
    let vp = opt_value(&ca, "vp", 6);
    let name = ca
        .optional_string("name", 7)
        .unwrap_or_else(|| auto_grob_name("GRID.points", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let pch = opt_value(&ca, "pch", 2);
    let size = opt_value(&ca, "size", 3);
    let gp = opt_value(&ca, "gp", 5);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("pch".to_string(), pch),
        ("size".to_string(), size),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("points", entries, draw, vp, context)
}

/// Draw a rectangle on the grid graphics device.
///
/// @param x x-coordinate of center
/// @param y y-coordinate of center
/// @param width rectangle width
/// @param height rectangle height
/// @param just justification
/// @param default.units default unit type
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a rect grob (invisibly)
#[interpreter_builtin(name = "grid.rect", namespace = "grid")]
fn interp_grid_rect(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 5)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 9, true);
    let vp = opt_value(&ca, "vp", 7);
    let name = ca
        .optional_string("name", 8)
        .unwrap_or_else(|| auto_grob_name("GRID.rect", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let width = ensure_unit(
        &ca.value("width", 2)
            .cloned()
            .unwrap_or_else(default_npc_one),
        &default_units,
    );
    let height = ensure_unit(
        &ca.value("height", 3)
            .cloned()
            .unwrap_or_else(default_npc_one),
        &default_units,
    );
    let just = opt_value(&ca, "just", 4);
    let gp = opt_value(&ca, "gp", 6);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("width".to_string(), width),
        ("height".to_string(), height),
        ("just".to_string(), just),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("rect", entries, draw, vp, context)
}

/// Draw a circle on the grid graphics device.
///
/// @param x x-coordinate of center
/// @param y y-coordinate of center
/// @param r radius
/// @param default.units default unit type
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a circle grob (invisibly)
#[interpreter_builtin(name = "grid.circle", namespace = "grid")]
fn interp_grid_circle(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 3)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 7, true);
    let vp = opt_value(&ca, "vp", 5);
    let name = ca
        .optional_string("name", 6)
        .unwrap_or_else(|| auto_grob_name("GRID.circle", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let r = ensure_unit(
        &ca.value("r", 2).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let gp = opt_value(&ca, "gp", 4);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("r".to_string(), r),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("circle", entries, draw, vp, context)
}

/// Draw a polygon on the grid graphics device.
///
/// @param x x-coordinates of vertices
/// @param y y-coordinates of vertices
/// @param default.units default unit type
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a polygon grob (invisibly)
#[interpreter_builtin(name = "grid.polygon", namespace = "grid")]
fn interp_grid_polygon(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 2)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 6, true);
    let vp = opt_value(&ca, "vp", 4);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.polygon", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let gp = opt_value(&ca, "gp", 3);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("polygon", entries, draw, vp, context)
}

/// Draw text on the grid graphics device.
///
/// @param label character string(s) to draw
/// @param x x-coordinate(s)
/// @param y y-coordinate(s)
/// @param just justification
/// @param rot rotation angle in degrees
/// @param default.units default unit type
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a text grob (invisibly)
#[interpreter_builtin(name = "grid.text", namespace = "grid")]
fn interp_grid_text(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 5)
        .unwrap_or_else(|| "npc".to_string());
    let draw = ca.logical_flag("draw", 9, true);
    let vp = opt_value(&ca, "vp", 7);
    let name = ca
        .optional_string("name", 8)
        .unwrap_or_else(|| auto_grob_name("GRID.text", context));

    let label = opt_value(&ca, "label", 0);
    let x = ensure_unit(
        &ca.value("x", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 2).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let just = opt_value(&ca, "just", 3);
    let rot = opt_value(&ca, "rot", 4);
    let gp = opt_value(&ca, "gp", 6);

    let entries = vec![
        ("label".to_string(), label),
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("just".to_string(), just),
        ("rot".to_string(), rot),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("text", entries, draw, vp, context)
}

// endregion

// region: Grob constructors (*Grob — create without drawing)

/// Create a text grob object without drawing it.
///
/// Equivalent to `grid.text(..., draw=FALSE)`.
///
/// @param label text to display
/// @param x,y position (unit or numeric in default.units)
/// @param just justification
/// @param rot rotation angle in degrees
/// @param gp graphical parameters (gpar)
/// @param name unique grob name
/// @param vp viewport
/// @return a text grob object
/// @namespace grid
#[interpreter_builtin(name = "textGrob", namespace = "grid")]
fn interp_text_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 5)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 7);
    let name = ca
        .optional_string("name", 8)
        .unwrap_or_else(|| auto_grob_name("GRID.text", context));

    let label = opt_value(&ca, "label", 0);
    let x = ensure_unit(
        &ca.value("x", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 2).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let just = normalize_just(&ca, "just", 3);
    let hjust = opt_value(&ca, "hjust", 9);
    let vjust = opt_value(&ca, "vjust", 10);
    let rot = opt_value(&ca, "rot", 4);
    let check_overlap = opt_value(&ca, "check.overlap", 11);
    let gp = opt_value(&ca, "gp", 6);

    let entries = vec![
        ("label".to_string(), label),
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("just".to_string(), just),
        ("hjust".to_string(), hjust),
        ("vjust".to_string(), vjust),
        ("rot".to_string(), rot),
        ("check.overlap".to_string(), check_overlap),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("text", entries, false, vp, context)
}

/// Create a lines grob object without drawing it.
///
/// @param x,y positions (unit or numeric)
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a lines grob object
/// @namespace grid
#[interpreter_builtin(name = "linesGrob", namespace = "grid")]
fn interp_lines_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 4)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 5);
    let name = ca
        .optional_string("name", 3)
        .unwrap_or_else(|| auto_grob_name("GRID.lines", context));

    let x = ensure_unit(
        &ca.value("x", 0)
            .cloned()
            .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into()))),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1)
            .cloned()
            .unwrap_or_else(|| RValue::vec(Vector::Double(vec![Some(0.0), Some(1.0)].into()))),
        &default_units,
    );
    let gp = opt_value(&ca, "gp", 2);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("lines", entries, false, vp, context)
}

/// Create a points grob object without drawing it.
///
/// @param x,y positions
/// @param pch point character
/// @param size point size
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a points grob object
/// @namespace grid
#[interpreter_builtin(name = "pointsGrob", namespace = "grid")]
fn interp_points_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 5)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 6);
    let name = ca
        .optional_string("name", 4)
        .unwrap_or_else(|| auto_grob_name("GRID.points", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let pch = opt_value(&ca, "pch", 2);
    let size = opt_value(&ca, "size", 3);
    let gp = opt_value(&ca, "gp", 7);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("pch".to_string(), pch),
        ("size".to_string(), size),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("points", entries, false, vp, context)
}

/// Create a rect grob object without drawing it.
///
/// @param x,y center position
/// @param width,height dimensions
/// @param just justification
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a rect grob object
/// @namespace grid
#[interpreter_builtin(name = "rectGrob", namespace = "grid")]
fn interp_rect_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 6)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 7);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.rect", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let width = opt_value(&ca, "width", 2);
    let height = opt_value(&ca, "height", 3);
    let just = normalize_just(&ca, "just", 4);
    let hjust = opt_value(&ca, "hjust", 9);
    let vjust = opt_value(&ca, "vjust", 10);
    let gp = opt_value(&ca, "gp", 8);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("width".to_string(), width),
        ("height".to_string(), height),
        ("just".to_string(), just),
        ("hjust".to_string(), hjust),
        ("vjust".to_string(), vjust),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("rect", entries, false, vp, context)
}

/// Create a circle grob object without drawing it.
///
/// @param x,y center position
/// @param r radius
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a circle grob object
/// @namespace grid
#[interpreter_builtin(name = "circleGrob", namespace = "grid")]
fn interp_circle_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 4)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 5);
    let name = ca
        .optional_string("name", 3)
        .unwrap_or_else(|| auto_grob_name("GRID.circle", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let r = opt_value(&ca, "r", 2);
    let gp = opt_value(&ca, "gp", 6);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("r".to_string(), r),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("circle", entries, false, vp, context)
}

/// Create a polygon grob object without drawing it.
///
/// @param x,y positions
/// @param id grouping indicator
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a polygon grob object
/// @namespace grid
#[interpreter_builtin(name = "polygonGrob", namespace = "grid")]
fn interp_polygon_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 4)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 5);
    let name = ca
        .optional_string("name", 3)
        .unwrap_or_else(|| auto_grob_name("GRID.polygon", context));

    let x = ensure_unit(
        &ca.value("x", 0).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let y = ensure_unit(
        &ca.value("y", 1).cloned().unwrap_or_else(default_npc_half),
        &default_units,
    );
    let id = opt_value(&ca, "id", 2);
    let gp = opt_value(&ca, "gp", 6);

    let entries = vec![
        ("x".to_string(), x),
        ("y".to_string(), y),
        ("id".to_string(), id),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("polygon", entries, false, vp, context)
}

/// Create a segments grob object without drawing it.
///
/// @param x0,y0 start positions
/// @param x1,y1 end positions
/// @param gp graphical parameters
/// @param name unique grob name
/// @param vp viewport
/// @return a segments grob object
/// @namespace grid
#[interpreter_builtin(name = "segmentsGrob", namespace = "grid")]
fn interp_segments_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let default_units = ca
        .optional_string("default.units", 6)
        .unwrap_or_else(|| "npc".to_string());
    let vp = opt_value(&ca, "vp", 7);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.segments", context));

    let x0 = ensure_unit(
        &ca.value("x0", 0).cloned().unwrap_or_else(default_npc_zero),
        &default_units,
    );
    let y0 = ensure_unit(
        &ca.value("y0", 1).cloned().unwrap_or_else(default_npc_zero),
        &default_units,
    );
    let x1 = ensure_unit(
        &ca.value("x1", 2).cloned().unwrap_or_else(default_npc_one),
        &default_units,
    );
    let y1 = ensure_unit(
        &ca.value("y1", 3).cloned().unwrap_or_else(default_npc_one),
        &default_units,
    );
    let gp = opt_value(&ca, "gp", 4);

    let entries = vec![
        ("x0".to_string(), x0),
        ("y0".to_string(), y0),
        ("x1".to_string(), x1),
        ("y1".to_string(), y1),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("segments", entries, false, vp, context)
}

/// Create a null grob (empty placeholder).
///
/// @param name unique grob name
/// @param vp viewport
/// @return a null grob object
/// @namespace grid
#[interpreter_builtin(name = "nullGrob", namespace = "grid")]
fn interp_null_grob(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let name = ca
        .optional_string("name", 0)
        .unwrap_or_else(|| auto_grob_name("GRID.null", context));

    let entries = vec![(
        "name".to_string(),
        RValue::vec(Vector::Character(vec![Some(name)].into())),
    )];

    make_grob("null", entries, false, RValue::Null, context)
}

/// Create a gTree (group of grobs).
///
/// @param children list of grobs (gList)
/// @param name unique grob name
/// @param gp graphical parameters
/// @param vp viewport
/// @return a gTree grob object
/// @namespace grid
#[interpreter_builtin(name = "gTree", namespace = "grid")]
fn interp_gtree(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let name = ca
        .optional_string("name", 1)
        .unwrap_or_else(|| auto_grob_name("GRID.gTree", context));
    let children = opt_value(&ca, "children", 0);
    let gp = opt_value(&ca, "gp", 2);
    let vp = opt_value(&ca, "vp", 3);

    let entries = vec![
        ("children".to_string(), children),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("gTree", entries, false, vp, context)
}

/// Create a gList (list of grobs).
///
/// @param ... grobs to combine
/// @return a gList object
/// @namespace grid
#[interpreter_builtin(name = "gList", namespace = "grid")]
fn interp_glist(
    args: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let entries: Vec<(Option<String>, RValue)> = args.iter().map(|a| (None, a.clone())).collect();
    let mut list = RList::new(entries);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(vec![Some("gList".to_string())].into())),
    );
    Ok(RValue::List(list))
}

// endregion

// region: Layout

/// Create a grid layout object specifying rows and columns.
///
/// @param nrow number of rows (default 1)
/// @param ncol number of columns (default 1)
/// @param widths column widths (unit object)
/// @param heights row heights (unit object)
/// @param respect logical or matrix controlling aspect ratio respect
/// @return a layout object (list with class "layout")
#[interpreter_builtin(name = "grid.layout", namespace = "grid")]
fn interp_grid_layout(
    args: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);

    let nrow = ca.integer_or("nrow", 0, 1);
    let ncol = ca.integer_or("ncol", 1, 1);
    let widths = opt_value(&ca, "widths", 2);
    let heights = opt_value(&ca, "heights", 3);
    let respect = opt_value(&ca, "respect", 4);

    // Default widths/heights: equal-sized units
    let widths = if matches!(widths, RValue::Null) {
        let vals: Vec<Option<f64>> = (0..ncol).map(|_| Some(1.0)).collect();
        make_grid_object(
            vec![
                (
                    "value".to_string(),
                    RValue::vec(Vector::Double(vals.into())),
                ),
                (
                    "units".to_string(),
                    RValue::vec(Vector::Character(
                        (0..ncol)
                            .map(|_| Some("null".to_string()))
                            .collect::<Vec<_>>()
                            .into(),
                    )),
                ),
            ],
            &["unit"],
        )
    } else {
        widths
    };

    let heights = if matches!(heights, RValue::Null) {
        let vals: Vec<Option<f64>> = (0..nrow).map(|_| Some(1.0)).collect();
        make_grid_object(
            vec![
                (
                    "value".to_string(),
                    RValue::vec(Vector::Double(vals.into())),
                ),
                (
                    "units".to_string(),
                    RValue::vec(Vector::Character(
                        (0..nrow)
                            .map(|_| Some("null".to_string()))
                            .collect::<Vec<_>>()
                            .into(),
                    )),
                ),
            ],
            &["unit"],
        )
    } else {
        heights
    };

    let entries = vec![
        (
            "nrow".to_string(),
            RValue::vec(Vector::Integer(vec![Some(nrow)].into())),
        ),
        (
            "ncol".to_string(),
            RValue::vec(Vector::Integer(vec![Some(ncol)].into())),
        ),
        ("widths".to_string(), widths),
        ("heights".to_string(), heights),
        ("respect".to_string(), respect),
    ];

    Ok(make_grid_object(entries, &["layout"]))
}

/// Visualize a grid layout by drawing labeled rectangles for each cell.
///
/// @param layout a layout object to visualize
/// @return NULL (invisibly)
#[interpreter_builtin(name = "grid.show.layout", namespace = "grid", min_args = 1)]
fn interp_grid_show_layout(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let layout = &args[0];

    // Extract nrow, ncol from the layout object
    let (nrow, ncol) = if let RValue::List(list) = layout {
        let mut nr = 1i64;
        let mut nc = 1i64;
        for (key, val) in &list.values {
            match key.as_deref() {
                Some("nrow") => {
                    if let Some(rv) = val.as_vector() {
                        nr = rv.as_integer_scalar().unwrap_or(1);
                    }
                }
                Some("ncol") => {
                    if let Some(rv) = val.as_vector() {
                        nc = rv.as_integer_scalar().unwrap_or(1);
                    }
                }
                _ => {}
            }
        }
        (nr.max(1) as usize, nc.max(1) as usize)
    } else {
        (1, 1)
    };

    // Draw a grid of rectangles with labels showing (row, col)
    let cell_width = 1.0 / ncol as f64;
    let cell_height = 1.0 / nrow as f64;

    for row in 0..nrow {
        for col in 0..ncol {
            let x_center = (col as f64 + 0.5) * cell_width;
            let y_center = 1.0 - (row as f64 + 0.5) * cell_height;

            // Draw the cell rectangle (Rust-level)
            let rect_gp = grid::gpar::Gpar {
                col: Some([0, 0, 0, 255]),
                fill: Some([255, 255, 255, 0]),
                lwd: Some(0.5),
                ..Default::default()
            };
            let rect_grob = grid::grob::Grob::Rect {
                x: grid::units::Unit::npc(x_center),
                y: grid::units::Unit::npc(y_center),
                width: grid::units::Unit::npc(cell_width),
                height: grid::units::Unit::npc(cell_height),
                just: (
                    grid::viewport::Justification::Centre,
                    grid::viewport::Justification::Centre,
                ),
                gp: rect_gp,
            };
            record_rust_grob(rect_grob, context);

            // Draw the cell label
            let label = format!("({}, {})", row + 1, col + 1);
            let text_gp = grid::gpar::Gpar {
                col: Some([100, 100, 100, 255]),
                fontsize: Some(8.0),
                ..Default::default()
            };
            let text_grob = grid::grob::Grob::Text {
                label: vec![label],
                x: grid::units::Unit::npc(x_center),
                y: grid::units::Unit::npc(y_center),
                just: (
                    grid::viewport::Justification::Centre,
                    grid::viewport::Justification::Centre,
                ),
                rot: 0.0,
                gp: text_gp,
            };
            record_rust_grob(text_grob, context);
        }
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

// endregion

// region: Grob manipulation

/// Retrieve a grob from the display list by name.
///
/// @param name character string naming the grob
/// @return the grob, or NULL if not found
#[interpreter_builtin(name = "grid.get", namespace = "grid", min_args = 1)]
fn interp_grid_get(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let target_name = ca.string("name", 0)?;

    let display_list = context.interpreter().grid_display_list.borrow();
    for item in display_list.iter() {
        if let RValue::List(list) = item {
            for (key, val) in &list.values {
                if key.as_deref() == Some("name") {
                    if let RValue::Vector(rv) = val {
                        if rv.inner.as_character_scalar().as_deref() == Some(target_name.as_str()) {
                            return Ok(item.clone());
                        }
                    }
                }
            }
        }
    }

    Ok(RValue::Null)
}

/// Modify properties of a grob on the display list.
///
/// @param name character string naming the grob
/// @param ... named arguments to update on the grob
/// @return NULL (invisibly)
#[interpreter_builtin(name = "grid.edit", namespace = "grid", min_args = 1)]
fn interp_grid_edit(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let target_name = ca.string("name", 0)?;

    let mut display_list = context.interpreter().grid_display_list.borrow_mut();
    for item in display_list.iter_mut() {
        if let RValue::List(list) = item {
            // Check if this grob has the target name
            let is_target = list.values.iter().any(|(key, val)| {
                key.as_deref() == Some("name")
                    && matches!(val, RValue::Vector(rv) if rv.inner.as_character_scalar().as_deref() == Some(target_name.as_str()))
            });

            if is_target {
                // Update properties from named args (skip "name" since that's the lookup key)
                for (key, val) in named {
                    if key == "name" {
                        continue;
                    }
                    // Find and replace existing entry, or append
                    let mut found = false;
                    for (entry_key, entry_val) in list.values.iter_mut() {
                        if entry_key.as_deref() == Some(key.as_str()) {
                            *entry_val = val.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        list.values.push((Some(key.clone()), val.clone()));
                    }
                }
                break;
            }
        }
    }

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

/// Remove a grob from the display list by name.
///
/// @param name character string naming the grob to remove
/// @return NULL (invisibly)
#[interpreter_builtin(name = "grid.remove", namespace = "grid", min_args = 1)]
fn interp_grid_remove(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let target_name = ca.string("name", 0)?;

    let mut display_list = context.interpreter().grid_display_list.borrow_mut();
    display_list.retain(|item| {
        if let RValue::List(list) = item {
            !list.values.iter().any(|(key, val)| {
                key.as_deref() == Some("name")
                    && matches!(val, RValue::Vector(rv) if rv.inner.as_character_scalar().as_deref() == Some(target_name.as_str()))
            })
        } else {
            true
        }
    });

    context.interpreter().set_invisible();
    Ok(RValue::Null)
}

// endregion

// region: Axes

/// Draw an x-axis on the grid graphics device.
///
/// @param at numeric vector of tick mark positions (in native coordinates)
/// @param label character vector of labels, or TRUE for automatic
/// @param main logical; if TRUE (default), draw below the viewport
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return an xaxis grob (invisibly)
#[interpreter_builtin(name = "grid.xaxis", namespace = "grid")]
fn interp_grid_xaxis(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let draw = ca.logical_flag("draw", 6, true);
    let vp = opt_value(&ca, "vp", 4);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.xaxis", context));

    let at = opt_value(&ca, "at", 0);
    let label = opt_value(&ca, "label", 1);
    let main = ca.logical_flag("main", 2, true);
    let gp = opt_value(&ca, "gp", 3);

    let entries = vec![
        ("at".to_string(), at),
        ("label".to_string(), label),
        (
            "main".to_string(),
            RValue::vec(Vector::Logical(vec![Some(main)].into())),
        ),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("xaxis", entries, draw, vp, context)
}

/// Draw a y-axis on the grid graphics device.
///
/// @param at numeric vector of tick mark positions (in native coordinates)
/// @param label character vector of labels, or TRUE for automatic
/// @param main logical; if TRUE (default), draw to the left of the viewport
/// @param gp graphical parameters
/// @param vp viewport to use
/// @param name grob name
/// @param draw whether to draw immediately (default TRUE)
/// @return a yaxis grob (invisibly)
#[interpreter_builtin(name = "grid.yaxis", namespace = "grid")]
fn interp_grid_yaxis(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(args, named);
    let draw = ca.logical_flag("draw", 6, true);
    let vp = opt_value(&ca, "vp", 4);
    let name = ca
        .optional_string("name", 5)
        .unwrap_or_else(|| auto_grob_name("GRID.yaxis", context));

    let at = opt_value(&ca, "at", 0);
    let label = opt_value(&ca, "label", 1);
    let main = ca.logical_flag("main", 2, true);
    let gp = opt_value(&ca, "gp", 3);

    let entries = vec![
        ("at".to_string(), at),
        ("label".to_string(), label),
        (
            "main".to_string(),
            RValue::vec(Vector::Logical(vec![Some(main)].into())),
        ),
        ("gp".to_string(), gp),
        (
            "name".to_string(),
            RValue::vec(Vector::Character(vec![Some(name)].into())),
        ),
    ];

    make_grob("yaxis", entries, draw, vp, context)
}

// endregion

// region: Grid-to-PlotState rendering

use crate::interpreter::graphics::plot_data::{PlotItem, PlotState};

/// Device dimensions in centimeters (default ~7 inches square).
const DEVICE_WIDTH_CM: f64 = 17.78;
const DEVICE_HEIGHT_CM: f64 = 17.78;

/// Convert the grid Rust display list into a `PlotState` for the existing
/// egui rendering pipeline.
///
/// This replays the display list through a `PlotStateRenderer` that converts
/// grid grobs (in cm coordinates) to `PlotItem`s in normalized [0,1] space.
fn grid_to_plot_state(
    display_list: &grid::display::DisplayList,
    grob_store: &grid::grob::GrobStore,
) -> PlotState {
    let mut renderer = PlotStateRenderer::new(DEVICE_WIDTH_CM, DEVICE_HEIGHT_CM);
    grid::render::replay(display_list, grob_store, &mut renderer);
    renderer.into_plot_state()
}

/// A `GridRenderer` implementation that converts grid drawing operations
/// into `PlotItem`s for the existing egui_plot rendering pipeline.
///
/// Coordinates are mapped from cm to normalized [0, device_size] coordinates,
/// then to a PlotState with x_lim and y_lim set to the device extent.
struct PlotStateRenderer {
    items: Vec<PlotItem>,
    device_width_cm: f64,
    device_height_cm: f64,
}

impl PlotStateRenderer {
    fn new(width_cm: f64, height_cm: f64) -> Self {
        PlotStateRenderer {
            items: Vec::new(),
            device_width_cm: width_cm,
            device_height_cm: height_cm,
        }
    }

    fn into_plot_state(self) -> PlotState {
        PlotState {
            items: self.items,
            title: None,
            x_label: None,
            y_label: None,
            x_lim: Some((0.0, self.device_width_cm)),
            y_lim: Some((0.0, self.device_height_cm)),
            show_legend: false,
        }
    }

    /// Convert RGBA with optional alpha to final RGBA.
    fn apply_alpha(rgba: [u8; 4], gp: &grid::gpar::Gpar) -> [u8; 4] {
        let alpha = gp.effective_alpha();
        if (alpha - 1.0).abs() < f64::EPSILON {
            rgba
        } else {
            let a = (f64::from(rgba[3]) * alpha) as u8;
            [rgba[0], rgba[1], rgba[2], a]
        }
    }
}

impl grid::render::GridRenderer for PlotStateRenderer {
    fn line(&mut self, x0_cm: f64, y0_cm: f64, x1_cm: f64, y1_cm: f64, gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        let width = gp.effective_lwd() as f32;
        self.items.push(PlotItem::Line {
            x: vec![x0_cm, x1_cm],
            y: vec![y0_cm, y1_cm],
            color: col,
            width,
            label: None,
        });
    }

    fn polyline(&mut self, x_cm: &[f64], y_cm: &[f64], gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        let width = gp.effective_lwd() as f32;
        self.items.push(PlotItem::Line {
            x: x_cm.to_vec(),
            y: y_cm.to_vec(),
            color: col,
            width,
            label: None,
        });
    }

    fn rect(&mut self, x_cm: f64, y_cm: f64, w_cm: f64, h_cm: f64, gp: &grid::gpar::Gpar) {
        let fill = gp.effective_fill();
        let col = Self::apply_alpha(gp.effective_col(), gp);
        let width = gp.effective_lwd() as f32;

        // Draw fill as a polygon if not transparent
        if fill[3] > 0 {
            // Use a line to represent the rectangle outline (4 corners + close)
            let fill_color = Self::apply_alpha(fill, gp);
            self.items.push(PlotItem::Line {
                x: vec![x_cm, x_cm + w_cm, x_cm + w_cm, x_cm, x_cm],
                y: vec![y_cm, y_cm, y_cm + h_cm, y_cm + h_cm, y_cm],
                color: fill_color,
                width: 0.5,
                label: None,
            });
        }

        // Draw outline
        if col[3] > 0 {
            self.items.push(PlotItem::Line {
                x: vec![x_cm, x_cm + w_cm, x_cm + w_cm, x_cm, x_cm],
                y: vec![y_cm, y_cm, y_cm + h_cm, y_cm + h_cm, y_cm],
                color: col,
                width,
                label: None,
            });
        }
    }

    fn circle(&mut self, x_cm: f64, y_cm: f64, r_cm: f64, gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        // Approximate circle with a polygon (24 segments)
        let n = 24;
        let mut xs = Vec::with_capacity(n + 1);
        let mut ys = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let theta = 2.0 * std::f64::consts::PI * (i as f64 / n as f64);
            xs.push(x_cm + r_cm * theta.cos());
            ys.push(y_cm + r_cm * theta.sin());
        }
        self.items.push(PlotItem::Line {
            x: xs,
            y: ys,
            color: col,
            width: gp.effective_lwd() as f32,
            label: None,
        });
    }

    fn polygon(&mut self, x_cm: &[f64], y_cm: &[f64], gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        let mut xs = x_cm.to_vec();
        let mut ys = y_cm.to_vec();
        // Close the polygon
        if let (Some(&first_x), Some(&first_y)) = (x_cm.first(), y_cm.first()) {
            xs.push(first_x);
            ys.push(first_y);
        }
        self.items.push(PlotItem::Line {
            x: xs,
            y: ys,
            color: col,
            width: gp.effective_lwd() as f32,
            label: None,
        });
    }

    fn text(&mut self, x_cm: f64, y_cm: f64, label: &str, _rot: f64, gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        self.items.push(PlotItem::Text {
            x: x_cm,
            y: y_cm,
            text: label.to_string(),
            color: col,
        });
    }

    fn point(&mut self, x_cm: f64, y_cm: f64, pch: u8, size_cm: f64, gp: &grid::gpar::Gpar) {
        let col = Self::apply_alpha(gp.effective_col(), gp);
        self.items.push(PlotItem::Points {
            x: vec![x_cm],
            y: vec![y_cm],
            color: col,
            size: size_cm as f32 * 4.0, // scale up for visibility
            shape: pch,
            label: None,
        });
    }

    fn clip(&mut self, _x_cm: f64, _y_cm: f64, _w_cm: f64, _h_cm: f64) {
        // Clipping is not supported in the PlotState model; ignore silently
    }

    fn unclip(&mut self) {
        // No-op
    }

    fn device_size_cm(&self) -> (f64, f64) {
        (self.device_width_cm, self.device_height_cm)
    }
}

/// Internal: flush the grid display list to a PlotState and send it to the
/// plot channel (if the plot feature is enabled).
fn flush_grid_to_plot(ctx: &BuiltinContext) {
    let dl = ctx.interpreter().grid_rust_display_list.borrow();
    if dl.is_empty() {
        return;
    }
    let store = ctx.interpreter().grid_grob_store.borrow();
    let plot_state = grid_to_plot_state(&dl, &store);
    drop(dl);
    drop(store);

    // Store it as the current_plot so existing flush_plot() picks it up
    *ctx.interpreter().current_plot.borrow_mut() = Some(plot_state);
}

/// Public API: flush any accumulated grid graphics to the GUI thread.
///
/// Called by the REPL loop after each eval to auto-display grid graphics,
/// alongside `flush_plot()` for base graphics.
pub fn flush_grid(interp: &crate::interpreter::Interpreter) {
    let dl = interp.grid_rust_display_list.borrow();
    if dl.is_empty() {
        return;
    }
    let store = interp.grid_grob_store.borrow();
    let plot_state = grid_to_plot_state(&dl, &store);
    drop(dl);
    drop(store);

    // If there's already a base-graphics plot, don't overwrite it — grid gets
    // its own turn only if no base plot is pending.
    if interp.current_plot.borrow().is_some() {
        return;
    }

    *interp.current_plot.borrow_mut() = Some(plot_state);
    // flush_plot() (called immediately after this) will send it to the GUI
}

// endregion
