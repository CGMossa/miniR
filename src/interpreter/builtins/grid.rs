//! Grid graphics builtins — R-facing functions for the grid graphics system.
//!
//! Grid objects are represented as R lists with S3 class attributes:
//! - Unit: `list(value=..., units=...)` with class "unit"
//! - Gpar: `list(col=..., fill=..., ...)` with class "gpar"
//! - Viewport: `list(x=..., y=..., width=..., ...)` with class "viewport"
//! - Grobs: `list(x=..., y=..., gp=..., ...)` with class c("<type>", "grob")

use super::CallArgs;
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
    context.interpreter().grid_display_list.borrow_mut().clear();
    context
        .interpreter()
        .grid_viewport_stack
        .borrow_mut()
        .clear();
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

    // Record the push on the display list
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

    // Record the pop on the display list
    drop(stack);
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
    let grob = make_grid_object(entries, &[type_class, "grob"]);

    if draw {
        match vp {
            RValue::Null => {
                record_on_display_list(&grob, ctx);
            }
            _ => {
                // Push viewport, draw, pop
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

                ctx.interpreter().grid_viewport_stack.borrow_mut().push(vp);

                record_on_display_list(&grob, ctx);

                ctx.interpreter().grid_viewport_stack.borrow_mut().pop();

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

/// Visualize a grid layout (stub).
///
/// @param layout a layout object to visualize
/// @return NULL (invisibly)
#[interpreter_builtin(name = "grid.show.layout", namespace = "grid")]
fn interp_grid_show_layout(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Stub — full visualization not yet implemented
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
