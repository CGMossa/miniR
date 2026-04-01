//! Interpreter-level builtins — functions that receive `BuiltinContext` so
//! they can call back into the active interpreter without direct TLS lookups.
//! Each is auto-registered via `#[interpreter_builtin]`.

use super::CallArgs;
use crate::interpreter::environment::Environment;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use crate::parser::ast::{Arg, BinaryOp, Expr, Param, UnaryOp};
use minir_macros::interpreter_builtin;

/// Extract `fail_fast` from named args and return the remaining named args.
/// Default is `false` (collect all errors).
fn extract_fail_fast(named: &[(String, RValue)]) -> (bool, Vec<(String, RValue)>) {
    let mut fail_fast = false;
    let mut remaining = Vec::with_capacity(named.len());
    for (name, val) in named {
        if name == "fail_fast" {
            fail_fast = val
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false);
        } else {
            remaining.push((name.clone(), val.clone()));
        }
    }
    (fail_fast, remaining)
}

/// Resolve a function specification: accepts an RValue::Function directly,
/// or a string naming a function to look up in the environment.
/// Equivalent to R's match.fun().
fn match_fun(f: &RValue, env: &Environment) -> Result<RValue, RError> {
    match f {
        RValue::Function(_) => Ok(f.clone()),
        RValue::Vector(rv) => match &rv.inner {
            Vector::Character(s) => {
                let name = s.first().and_then(|x| x.as_ref()).ok_or_else(|| {
                    RError::new(
                        RErrorKind::Argument,
                        "not a valid function name".to_string(),
                    )
                })?;
                env.get_function(name)
                    .ok_or_else(|| RError::other(format!("could not find function '{}'", name)))
            }
            _ => Err(RError::new(
                RErrorKind::Argument,
                "FUN is not a function and not a string naming a function".to_string(),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Argument,
            "FUN is not a function and not a string naming a function".to_string(),
        )),
    }
}

fn optional_frame_index(positional: &[RValue], default: i64) -> Result<i64, RError> {
    match positional.first() {
        None => Ok(default),
        Some(value) => value
            .as_vector()
            .and_then(|v| v.as_integer_scalar())
            .ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "frame index must be an integer".to_string(),
                )
            }),
    }
}

fn language_or_null(expr: Option<crate::parser::ast::Expr>) -> RValue {
    match expr {
        Some(expr) => RValue::Language(Language::new(expr)),
        None => RValue::Null,
    }
}

// region: S3-dispatching generics (print, format)

/// Get explicit class attributes from an RValue.
/// Returns an empty vec for objects without a class attribute.
fn explicit_classes(val: &RValue) -> Vec<String> {
    match val {
        RValue::Vector(rv) => rv.class().unwrap_or_default(),
        RValue::List(list) => {
            if let Some(RValue::Vector(rv)) = list.get_attr("class") {
                if let Vector::Character(classes) = &rv.inner {
                    classes.iter().filter_map(|c| c.clone()).collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        RValue::Language(lang) => lang.class().unwrap_or_default(),
        _ => vec![],
    }
}

/// Try S3 dispatch for a generic function. Returns `Ok(Some(result))` if a
/// method was found and called, `Ok(None)` if no method exists (caller should
/// fall through to default behavior).
fn try_s3_dispatch(
    generic: &str,
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<Option<RValue>, RError> {
    let Some(val) = args.first() else {
        return Ok(None);
    };
    let classes = explicit_classes(val);
    if classes.is_empty() {
        return Ok(None);
    }
    let env = context.env();
    let interp = context.interpreter();

    // First pass: look up generic.class in the environment chain
    for class in &classes {
        let method_name = format!("{generic}.{class}");
        if let Some(method) = env.get(&method_name) {
            let result = interp.call_function(&method, args, named, env)?;
            return Ok(Some(result));
        }
    }

    // Second pass: check the per-interpreter S3 method registry
    for class in &classes {
        if let Some(method) = interp.lookup_s3_method(generic, class) {
            let result = interp.call_function(&method, args, named, env)?;
            return Ok(Some(result));
        }
    }

    Ok(None)
}

/// Print a value to stdout (S3 generic).
///
/// @param x the value to print
/// @return x, invisibly
#[interpreter_builtin(min_args = 1)]
fn interp_print(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (print.Date, print.POSIXct, print.data.frame, etc.)
    if let Some(result) = try_s3_dispatch("print", args, named, context)? {
        return Ok(result);
    }
    // Default print
    if let Some(val) = args.first() {
        context.write(&format!("{}\n", val));
    }
    // print() returns invisibly in R
    context.interpreter().set_invisible();
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// Return a value invisibly (suppresses auto-printing).
///
/// Sets the interpreter's visibility flag so that the REPL/eval loop
/// knows not to auto-print the result.
///
/// @param x value to return (default: NULL)
/// @return x (invisibly)
#[interpreter_builtin]
fn interp_invisible(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.interpreter().set_invisible();
    Ok(args.first().cloned().unwrap_or(RValue::Null))
}

/// Format a value as a character string (S3 generic).
///
/// Supports named parameters: `nsmall` (minimum decimal places for doubles),
/// `width` (minimum field width, right-justified), `big.mark` (thousands
/// separator), and `scientific` (force scientific notation when TRUE, suppress
/// when FALSE).
///
/// @param x the value to format
/// @param nsmall minimum number of digits to the right of the decimal point
/// @param width minimum field width (right-justified with spaces)
/// @param big.mark character to insert as thousands separator
/// @param scientific logical; TRUE forces scientific notation, FALSE suppresses it
/// @return character vector representation
#[interpreter_builtin(min_args = 1)]
fn interp_format(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (format.Date, format.POSIXct, etc.)
    if let Some(result) = try_s3_dispatch("format", args, named, context)? {
        return Ok(result);
    }

    // Extract named parameters
    let nsmall: Option<usize> = named
        .iter()
        .find(|(k, _)| k == "nsmall")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok());
    let digits: Option<usize> = named
        .iter()
        .find(|(k, _)| k == "digits")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok());
    let width: Option<usize> = named
        .iter()
        .find(|(k, _)| k == "width")
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .and_then(|i| usize::try_from(i).ok());
    let big_mark: Option<String> = named
        .iter()
        .find(|(k, _)| k == "big.mark")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar());
    let scientific: Option<bool> = named
        .iter()
        .find(|(k, _)| k == "scientific")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar());

    let has_format_opts = nsmall.is_some()
        || digits.is_some()
        || width.is_some()
        || big_mark.is_some()
        || scientific.is_some();

    match args.first() {
        Some(RValue::Vector(rv)) if has_format_opts => {
            let formatted: Vec<Option<String>> = match &rv.inner {
                Vector::Double(vals) => vals
                    .iter_opt()
                    .map(|x| {
                        x.map(|f| {
                            format_double_with_opts(
                                f,
                                nsmall,
                                digits,
                                big_mark.as_deref(),
                                scientific,
                            )
                        })
                    })
                    .collect(),
                Vector::Integer(vals) => vals
                    .iter_opt()
                    .map(|x| x.map(|i| format_integer_with_opts(i, big_mark.as_deref())))
                    .collect(),
                other => other.to_characters(),
            };
            // Apply width padding if requested
            let formatted = if let Some(w) = width {
                formatted
                    .into_iter()
                    .map(|s| s.map(|s| format!("{:>width$}", s, width = w)))
                    .collect()
            } else {
                formatted
            };
            Ok(RValue::vec(Vector::Character(formatted.into())))
        }
        Some(RValue::Vector(rv)) => {
            // No special formatting options — element-wise default format
            let chars = rv.inner.to_characters();
            let chars = if let Some(w) = width {
                chars
                    .into_iter()
                    .map(|s| s.map(|s| format!("{:>width$}", s, width = w)))
                    .collect()
            } else {
                chars
            };
            Ok(RValue::vec(Vector::Character(chars.into())))
        }
        Some(val) => Ok(RValue::vec(Vector::Character(
            vec![Some(format!("{}", val))].into(),
        ))),
        None => Ok(RValue::vec(Vector::Character(
            vec![Some(String::new())].into(),
        ))),
    }
}

/// Format a double value with nsmall, digits, big.mark, and scientific options.
fn format_double_with_opts(
    f: f64,
    nsmall: Option<usize>,
    digits: Option<usize>,
    big_mark: Option<&str>,
    scientific: Option<bool>,
) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }

    let s = match scientific {
        Some(true) => {
            if let Some(d) = digits {
                // digits controls significant digits; in scientific notation
                // that means d-1 decimal places after the leading digit
                format!("{:.prec$e}", f, prec = d.saturating_sub(1))
            } else {
                format!("{:e}", f)
            }
        }
        Some(false) => {
            // Suppress scientific notation
            if let Some(ns) = nsmall {
                format!("{:.prec$}", f, prec = ns)
            } else if let Some(d) = digits {
                format_significant_digits(f, d)
            } else if f == f.floor() && f.abs() < 1e15 {
                use crate::interpreter::coerce;
                format!("{}", coerce::f64_to_i64(f).unwrap_or(0))
            } else {
                format!("{}", f)
            }
        }
        None => {
            if let Some(ns) = nsmall {
                format!("{:.prec$}", f, prec = ns)
            } else if let Some(d) = digits {
                format_significant_digits(f, d)
            } else {
                use crate::interpreter::value::vector::format_r_double;
                format_r_double(f)
            }
        }
    };

    match big_mark {
        Some(mark) if !mark.is_empty() => insert_thousands_sep(&s, mark),
        _ => s,
    }
}

/// Format a double value to a specified number of significant digits.
fn format_significant_digits(f: f64, digits: usize) -> String {
    if f == 0.0 {
        return format!("{:.prec$}", 0.0, prec = digits.saturating_sub(1));
    }
    let magnitude = f.abs().log10().floor() as i32;
    let decimal_places = (i64::from(digits as i32) - 1 - i64::from(magnitude)).max(0);
    let decimal_places = usize::try_from(decimal_places).unwrap_or(0);
    format!("{:.prec$}", f, prec = decimal_places)
}

/// Format an integer value with big.mark option.
fn format_integer_with_opts(i: i64, big_mark: Option<&str>) -> String {
    let s = i.to_string();
    match big_mark {
        Some(mark) if !mark.is_empty() => insert_thousands_sep(&s, mark),
        _ => s,
    }
}

/// Insert a thousands separator into the integer part of a numeric string.
fn insert_thousands_sep(s: &str, sep: &str) -> String {
    // Split into sign, integer part, and decimal part
    let (sign, rest) = if let Some(stripped) = s.strip_prefix('-') {
        ("-", stripped)
    } else {
        ("", s)
    };

    let (int_part, dec_part) = match rest.find('.') {
        Some(pos) => (&rest[..pos], Some(&rest[pos..])),
        None => (rest, None),
    };

    // Insert separator every 3 digits from the right
    let digits: Vec<char> = int_part.chars().collect();
    let mut result = String::with_capacity(int_part.len() + (int_part.len() / 3) * sep.len());
    for (i, ch) in digits.iter().enumerate() {
        let remaining = digits.len() - i;
        if i > 0 && remaining.is_multiple_of(3) {
            result.push_str(sep);
        }
        result.push(*ch);
    }

    let mut out = String::from(sign);
    out.push_str(&result);
    if let Some(dec) = dec_part {
        out.push_str(dec);
    }
    out
}

/// Print a data.frame with aligned columns using TabWriter.
///
/// @param x a data.frame to print
/// @return x, invisibly
#[cfg(feature = "tables")]
#[interpreter_builtin(name = "print.data.frame", min_args = 1)]
fn interp_print_data_frame(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    use std::io::Write;
    use tabwriter::TabWriter;

    let val = &args[0];
    let list = match val {
        RValue::List(l) => l,
        _ => {
            context.write(&format!("{}\n", val));
            return Ok(val.clone());
        }
    };

    if list.values.is_empty() {
        context.write("data frame with 0 columns and 0 rows\n");
        return Ok(val.clone());
    }

    // Column names
    let col_names: Vec<String> = list
        .values
        .iter()
        .enumerate()
        .map(|(i, (name, _))| name.clone().unwrap_or_else(|| format!("V{}", i + 1)))
        .collect();

    // Number of rows: from row.names attribute or first column length
    let nrow = list
        .get_attr("row.names")
        .map(|v| v.length())
        .unwrap_or_else(|| list.values.first().map(|(_, v)| v.length()).unwrap_or(0));

    if nrow == 0 {
        // Print header only for 0-row data frames
        context.write(&format!(
            "data frame with 0 rows and {} columns: {}\n",
            col_names.len(),
            col_names.join(", ")
        ));
        return Ok(val.clone());
    }

    // Row names
    let row_names: Vec<String> = match list.get_attr("row.names") {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Character(chars) => chars
                .iter()
                .map(|c| c.clone().unwrap_or_else(|| "NA".to_string()))
                .collect(),
            Vector::Integer(ints) => ints
                .iter()
                .map(|i| match i {
                    Some(v) => v.to_string(),
                    None => "NA".to_string(),
                })
                .collect(),
            _ => (1..=nrow).map(|i| i.to_string()).collect(),
        },
        _ => (1..=nrow).map(|i| i.to_string()).collect(),
    };

    // Format each column's elements
    let formatted_cols: Vec<Vec<String>> = list
        .values
        .iter()
        .map(|(_, value)| match value {
            RValue::Vector(rv) => format_column_elements(&rv.inner, nrow),
            RValue::Null => vec!["NULL".to_string(); nrow],
            other => vec![format!("{}", other); nrow],
        })
        .collect();

    // Build tab-separated output and let tabwriter align it
    let mut tw = TabWriter::new(Vec::new());

    // Header row: blank for row-names column, then column names
    let header_parts: Vec<&str> = std::iter::once("")
        .chain(col_names.iter().map(|s| s.as_str()))
        .collect();
    writeln!(tw, "{}", header_parts.join("\t"))
        .map_err(|e| RError::other(format!("write error: {}", e)))?;

    // Data rows
    for row in 0..nrow {
        let row_name = row_names.get(row).map(|s| s.as_str()).unwrap_or("");
        let mut parts = vec![row_name.to_string()];
        for col in &formatted_cols {
            parts.push(col.get(row).cloned().unwrap_or_else(|| "NA".to_string()));
        }
        writeln!(tw, "{}", parts.join("\t"))
            .map_err(|e| RError::other(format!("write error: {}", e)))?;
    }

    tw.flush()
        .map_err(|e| RError::other(format!("flush error: {}", e)))?;
    let output = String::from_utf8(tw.into_inner().unwrap_or_default())
        .map_err(|e| RError::other(format!("utf8 error: {}", e)))?;

    // Print without trailing newline
    context.write(&output);

    Ok(val.clone())
}

#[cfg(feature = "tables")]
/// Format individual elements of a vector column for data frame printing.
fn format_column_elements(v: &Vector, nrow: usize) -> Vec<String> {
    let len = v.len();
    (0..nrow)
        .map(|i| {
            if i >= len {
                return "NA".to_string();
            }
            match v {
                Vector::Raw(vals) => format!("{:02x}", vals[i]),
                Vector::Logical(vals) => match vals[i] {
                    Some(true) => "TRUE".to_string(),
                    Some(false) => "FALSE".to_string(),
                    None => "NA".to_string(),
                },
                Vector::Integer(vals) => match vals.get_opt(i) {
                    Some(n) => n.to_string(),
                    None => "NA".to_string(),
                },
                Vector::Double(vals) => match vals.get_opt(i) {
                    Some(f) => format_r_double(f),
                    None => "NA".to_string(),
                },
                Vector::Complex(vals) => match vals[i] {
                    Some(c) => format_r_complex(c),
                    None => "NA".to_string(),
                },
                Vector::Character(vals) => match &vals[i] {
                    Some(s) => s.clone(),
                    None => "NA".to_string(),
                },
            }
        })
        .collect()
}

/// Print a matrix with row/column labels.
///
/// Formats the vector data as a 2D grid using the `dim` attribute for layout
/// and `dimnames` for row/column labels.
///
/// @param x a matrix to print
/// @return x, invisibly
#[interpreter_builtin(name = "print.matrix", min_args = 1)]
fn interp_print_matrix(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let val = &args[0];
    let rv = match val {
        RValue::Vector(rv) => rv,
        _ => {
            context.write(&format!("{}\n", val));
            return Ok(val.clone());
        }
    };

    // Extract dim attribute
    let (nrow, ncol) = match rv.get_attr("dim") {
        Some(RValue::Vector(dim_rv)) => {
            let dims = dim_rv.to_integers();
            if dims.len() == 2 {
                let nr = dims[0].unwrap_or(0);
                let nc = dims[1].unwrap_or(0);
                (
                    usize::try_from(nr).unwrap_or(0),
                    usize::try_from(nc).unwrap_or(0),
                )
            } else {
                context.write(&format!("{}\n", val));
                return Ok(val.clone());
            }
        }
        _ => {
            context.write(&format!("{}\n", val));
            return Ok(val.clone());
        }
    };

    // Extract dimnames
    let (row_names, col_names) = extract_dimnames(rv);

    // Format each element
    let elements = format_matrix_elements(&rv.inner, nrow, ncol);

    // Compute column widths (including headers)
    let col_widths: Vec<usize> = (0..ncol)
        .map(|j| {
            let header_w = col_names
                .as_ref()
                .and_then(|cn| cn.get(j))
                .map(|s| s.len())
                .unwrap_or_else(|| format!("[,{}]", j + 1).len());
            let max_elem_w = (0..nrow)
                .map(|i| elements[i * ncol + j].len())
                .max()
                .unwrap_or(0);
            header_w.max(max_elem_w)
        })
        .collect();

    // Row label width
    let row_label_width = (0..nrow)
        .map(|i| {
            row_names
                .as_ref()
                .and_then(|rn| rn.get(i))
                .map(|s| s.len())
                .unwrap_or_else(|| format!("[{},]", i + 1).len())
        })
        .max()
        .unwrap_or(0);

    // Also account for the blank space above row labels in the header line
    let mut output = String::new();

    // Header line
    output.push_str(&" ".repeat(row_label_width));
    for (j, &cw) in col_widths.iter().enumerate() {
        let header = col_names
            .as_ref()
            .and_then(|cn| cn.get(j).cloned())
            .unwrap_or_else(|| format!("[,{}]", j + 1));
        output.push(' ');
        output.push_str(&format!("{:>width$}", header, width = cw));
    }
    output.push('\n');

    // Data rows
    for i in 0..nrow {
        let row_label = row_names
            .as_ref()
            .and_then(|rn| rn.get(i).cloned())
            .unwrap_or_else(|| format!("[{},]", i + 1));
        output.push_str(&format!("{:>width$}", row_label, width = row_label_width));
        for j in 0..ncol {
            output.push(' ');
            output.push_str(&format!(
                "{:>width$}",
                elements[i * ncol + j],
                width = col_widths[j]
            ));
        }
        output.push('\n');
    }

    context.write(&output);
    context.interpreter().set_invisible();
    Ok(val.clone())
}

/// Extract dimnames from a matrix's attributes.
/// Returns (row_names, col_names) as optional Vec<String>.
fn extract_dimnames(rv: &RVector) -> (Option<Vec<String>>, Option<Vec<String>>) {
    match rv.get_attr("dimnames") {
        Some(RValue::List(list)) => {
            let row_names = list.values.first().and_then(|(_, v)| match v {
                RValue::Vector(rv) => {
                    if let Vector::Character(chars) = &rv.inner {
                        Some(
                            chars
                                .iter()
                                .map(|c| c.clone().unwrap_or_else(|| "NA".to_string()))
                                .collect(),
                        )
                    } else {
                        None
                    }
                }
                _ => None,
            });
            let col_names = list.values.get(1).and_then(|(_, v)| match v {
                RValue::Vector(rv) => {
                    if let Vector::Character(chars) = &rv.inner {
                        Some(
                            chars
                                .iter()
                                .map(|c| c.clone().unwrap_or_else(|| "NA".to_string()))
                                .collect(),
                        )
                    } else {
                        None
                    }
                }
                _ => None,
            });
            (row_names, col_names)
        }
        _ => (None, None),
    }
}

/// Format matrix elements as strings, stored in row-major order.
fn format_matrix_elements(v: &Vector, nrow: usize, ncol: usize) -> Vec<String> {
    let len = v.len();
    // R stores matrices in column-major order: element [i,j] is at index i + j*nrow
    (0..nrow * ncol)
        .map(|idx| {
            let i = idx / ncol; // row
            let j = idx % ncol; // col
            let flat_idx = i + j * nrow; // column-major index
            if flat_idx >= len {
                return "NA".to_string();
            }
            match v {
                Vector::Raw(vals) => format!("{:02x}", vals[flat_idx]),
                Vector::Logical(vals) => match vals[flat_idx] {
                    Some(true) => "TRUE".to_string(),
                    Some(false) => "FALSE".to_string(),
                    None => "NA".to_string(),
                },
                Vector::Integer(vals) => match vals.get_opt(flat_idx) {
                    Some(n) => n.to_string(),
                    None => "NA".to_string(),
                },
                Vector::Double(vals) => match vals.get_opt(flat_idx) {
                    Some(f) => format_r_double(f),
                    None => "NA".to_string(),
                },
                Vector::Complex(vals) => match vals[flat_idx] {
                    Some(c) => format_r_complex(c),
                    None => "NA".to_string(),
                },
                Vector::Character(vals) => match &vals[flat_idx] {
                    Some(s) => format!("\"{}\"", s),
                    None => "NA".to_string(),
                },
            }
        })
        .collect()
}

/// Print a factor showing level labels instead of integer codes.
///
/// @param x a factor to print
/// @return x, invisibly
#[interpreter_builtin(name = "print.factor", min_args = 1)]
fn interp_print_factor(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let val = &args[0];
    let rv = match val {
        RValue::Vector(rv) => rv,
        _ => {
            context.write(&format!("{}\n", val));
            return Ok(val.clone());
        }
    };

    // Get levels
    let levels: Vec<String> = match rv.get_attr("levels") {
        Some(RValue::Vector(lv)) => match &lv.inner {
            Vector::Character(chars) => chars
                .iter()
                .map(|c| c.clone().unwrap_or_else(|| "NA".to_string()))
                .collect(),
            _ => vec![],
        },
        _ => vec![],
    };

    // Map integer codes to level labels
    let labels: Vec<String> = rv
        .to_integers()
        .iter()
        .map(|code| match code {
            Some(i) => {
                let idx = usize::try_from(*i).ok().and_then(|i| i.checked_sub(1));
                match idx {
                    Some(idx) if idx < levels.len() => levels[idx].clone(),
                    _ => "NA".to_string(),
                }
            }
            None => "NA".to_string(),
        })
        .collect();

    // Format like a character vector with [1] prefix
    let formatted = format_factor_labels(&labels);
    context.write(&formatted);
    context.write(&format!("Levels: {}\n", levels.join(" ")));

    context.interpreter().set_invisible();
    Ok(val.clone())
}

/// Format factor labels as R-style output with line indices.
fn format_factor_labels(labels: &[String]) -> String {
    if labels.is_empty() {
        return "factor(0)\n".to_string();
    }

    let max_width = 80;
    let mut result = String::new();
    let mut pos = 0;

    while pos < labels.len() {
        let label = format!("[{}]", pos + 1);
        let label_width = label.len();
        let mut line = format!("{} ", label);
        let mut current_width = label_width + 1;
        let line_start = pos;

        while pos < labels.len() {
            let elem = &labels[pos];
            let elem_width = elem.len() + 1; // +1 for space
            if current_width + elem_width > max_width && pos > line_start {
                break;
            }
            line.push_str(elem);
            if pos + 1 < labels.len() {
                line.push(' ');
            }
            current_width += elem_width;
            pos += 1;
        }

        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&line);
    }

    result.push('\n');
    result
}

// endregion

/// Apply a function over a vector or list, simplifying the result.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @return simplified vector or list of results
#[interpreter_builtin(name = "sapply", min_args = 2)]
fn interp_sapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, true, context)
}

/// Apply a function over a vector or list, returning a list.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @param ... additional arguments passed to FUN
/// @return list of results
#[interpreter_builtin(name = "lapply", min_args = 2)]
fn interp_lapply(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_apply(args, named, false, context)
}

/// Apply a function over a vector or list with a type-checked return template.
///
/// vapply is similar to sapply, but requires a FUN.VALUE template that specifies
/// the expected return type and length. Each result is checked against the template,
/// and an error is raised if there is a mismatch.
///
/// @param X vector or list to iterate over
/// @param FUN function to apply to each element
/// @param FUN.VALUE template value specifying the expected return type and length
/// @param ... additional arguments passed to FUN
/// @return simplified vector matching FUN.VALUE type
#[interpreter_builtin(name = "vapply", min_args = 3)]
fn interp_vapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ca = CallArgs::new(positional, named);
    let env = context.env();

    let x = ca
        .value("X", 0)
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "vapply requires at least 3 arguments: X, FUN, and FUN.VALUE",
            )
        })?
        .clone();
    let f_val = ca
        .value("FUN", 1)
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "vapply requires at least 3 arguments: X, FUN, and FUN.VALUE",
            )
        })?
        .clone();
    let f = match_fun(&f_val, env)?;
    let fun_value = ca
        .value("FUN.VALUE", 2)
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "vapply requires at least 3 arguments: X, FUN, and FUN.VALUE",
            )
        })?
        .clone();

    // Filter out vapply's own named args before passing the rest to FUN
    let vapply_params = ["X", "FUN", "FUN.VALUE", "USE.NAMES", "fail_fast"];
    let (fail_fast, _) = extract_fail_fast(named);
    let extra_named: Vec<(String, RValue)> = named
        .iter()
        .filter(|(n, _)| !vapply_params.contains(&n.as_str()))
        .cloned()
        .collect();

    // Determine expected type and length from FUN.VALUE
    let expected_len = fun_value.length();
    let expected_type = fun_value.type_name().to_string();

    let items: Vec<RValue> = rvalue_to_items(&x);

    // Extra positional args beyond X, FUN, FUN.VALUE are passed to FUN
    let extra_args: Vec<RValue> = positional.iter().skip(3).cloned().collect();
    context.with_interpreter(|interp| {
        let mut results: Vec<RValue> = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let mut call_args = vec![item.clone()];
            call_args.extend(extra_args.iter().cloned());
            let result = if fail_fast {
                interp.call_function(&f, &call_args, &extra_named, env)?
            } else {
                interp
                    .call_function(&f, &call_args, &extra_named, env)
                    .unwrap_or(RValue::Null)
            };

            // Validate result matches FUN.VALUE template
            let result_len = result.length();
            let result_type = result.type_name().to_string();
            if result_len != expected_len {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "values must be length {} (FUN.VALUE), but FUN(X[[{}]]) result is length {}",
                        expected_len,
                        i + 1,
                        result_len
                    ),
                ));
            }
            if result_type != expected_type {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!(
                        "values must be type '{}' (FUN.VALUE), but FUN(X[[{}]]) result is type '{}'",
                        expected_type,
                        i + 1,
                        result_type
                    ),
                ));
            }
            results.push(result);
        }

        // Simplify results: vapply always simplifies since we've validated types
        if results.is_empty() {
            // Return an empty vector of the expected type
            return match expected_type.as_str() {
                "double" => Ok(RValue::vec(Vector::Double(vec![].into()))),
                "integer" => Ok(RValue::vec(Vector::Integer(vec![].into()))),
                "character" => Ok(RValue::vec(Vector::Character(vec![].into()))),
                "logical" => Ok(RValue::vec(Vector::Logical(vec![].into()))),
                _ => Ok(RValue::List(RList::new(vec![]))),
            };
        }

        if expected_len == 1 {
            // Scalar results: simplify to a typed vector
            match expected_type.as_str() {
                "double" => {
                    let vals: Vec<Option<f64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    Ok(RValue::vec(Vector::Double(vals.into())))
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    Ok(RValue::vec(Vector::Integer(vals.into())))
                }
                "character" => {
                    let vals: Vec<Option<String>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    Ok(RValue::vec(Vector::Character(vals.into())))
                }
                "logical" => {
                    let vals: Vec<Option<bool>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    Ok(RValue::vec(Vector::Logical(vals.into())))
                }
                _ => {
                    let values: Vec<(Option<String>, RValue)> =
                        results.into_iter().map(|v| (None, v)).collect();
                    Ok(RValue::List(RList::new(values)))
                }
            }
        } else {
            // Multi-value results: build a matrix (each result becomes a column)
            simplify_apply_results(results)
        }
    })
}

fn eval_apply(
    positional: &[RValue],
    named: &[(String, RValue)],
    simplify: bool,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "need at least 2 arguments for apply".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let x = &positional[0];
    let f = match_fun(&positional[1], env)?;

    let items: Vec<RValue> = rvalue_to_items(x);

    // Extra positional args beyond X and FUN are passed to FUN in each call
    let extra_args: Vec<RValue> = positional.iter().skip(2).cloned().collect();

    let env = context.env();
    context.with_interpreter(|interp| {
        let mut results: Vec<RValue> = Vec::new();
        for item in &items {
            let mut call_args = vec![item.clone()];
            call_args.extend(extra_args.iter().cloned());
            if fail_fast {
                let result = interp.call_function(&f, &call_args, &extra_named, env)?;
                results.push(result);
            } else {
                match interp.call_function(&f, &call_args, &extra_named, env) {
                    Ok(result) => results.push(result),
                    Err(_) => results.push(RValue::Null),
                }
            }
        }

        if simplify {
            let all_scalar = results.iter().all(|r| r.length() == 1);
            if all_scalar && !results.is_empty() {
                let first_type = results[0].type_name();
                let all_same = results.iter().all(|r| r.type_name() == first_type);
                if all_same {
                    match first_type {
                        "double" => {
                            let vals: Vec<Option<f64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Double(vals.into())));
                        }
                        "integer" => {
                            let vals: Vec<Option<i64>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Integer(vals.into())));
                        }
                        "character" => {
                            let vals: Vec<Option<String>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector().map(|v| {
                                        v.to_characters().into_iter().next().unwrap_or(None)
                                    })
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Character(vals.into())));
                        }
                        "logical" => {
                            let vals: Vec<Option<bool>> = results
                                .iter()
                                .filter_map(|r| {
                                    r.as_vector()
                                        .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                                })
                                .collect();
                            return Ok(RValue::vec(Vector::Logical(vals.into())));
                        }
                        _ => {}
                    }
                }
            }
        }

        let values: Vec<(Option<String>, RValue)> =
            results.into_iter().map(|v| (None, v)).collect();
        Ok(RValue::List(RList::new(values)))
    })
}

/// Call a function with arguments supplied as a list.
///
/// Named elements in the list are passed as named arguments to the function.
/// Unnamed elements are passed as positional arguments.
///
/// @param what function or character string naming the function
/// @param args list of arguments to pass to the function
/// @param quote if TRUE, do not evaluate the arguments (default FALSE)
/// @return the result of the function call
#[interpreter_builtin(name = "do.call", min_args = 2)]
fn interp_do_call(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    if positional.len() >= 2 {
        let f = match_fun(&positional[0], env)?;

        // Get the target environment from envir= named arg (defaults to calling env)
        let target_env = named
            .iter()
            .find(|(n, _)| n == "envir")
            .and_then(|(_, v)| {
                if let RValue::Environment(e) = v {
                    Some(e.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| env.clone());

        // Filter out the envir= arg before forwarding to the function
        let forwarded_named: Vec<(String, RValue)> = named
            .iter()
            .filter(|(n, _)| n != "envir")
            .cloned()
            .collect();

        // Handle pre-eval builtins that can't go through the normal dispatch path
        if let RValue::Function(crate::interpreter::value::RFunction::Builtin {
            name: builtin_name,
            ..
        }) = &f
        {
            if builtin_name == "on.exit" {
                return do_call_on_exit(&positional[1], &forwarded_named, &target_env);
            }
        }

        return context.with_interpreter(|interp| match &positional[1] {
            RValue::List(l) => {
                // Split list elements into positional and named args
                let mut pos_args: Vec<RValue> = Vec::new();
                let mut named_args: Vec<(String, RValue)> = forwarded_named;
                for (name, value) in &l.values {
                    match name {
                        Some(n) if !n.is_empty() => {
                            named_args.push((n.clone(), value.clone()));
                        }
                        _ => {
                            pos_args.push(value.clone());
                        }
                    }
                }
                interp
                    .call_function(&f, &pos_args, &named_args, &target_env)
                    .map_err(RError::from)
            }
            _ => interp
                .call_function(&f, &positional[1..], &forwarded_named, &target_env)
                .map_err(RError::from),
        });
    }
    Err(RError::new(
        RErrorKind::Argument,
        "do.call requires at least 2 arguments".to_string(),
    ))
}

/// Handle `do.call(on.exit, list(expr, add), envir=env)`.
///
/// `on.exit` is a pre-eval builtin that stores unevaluated expressions.
/// When called via `do.call`, the expression is already an RValue (typically
/// a Language/call object). We convert it back to an Expr for storage.
fn do_call_on_exit(
    args_val: &RValue,
    named: &[(String, RValue)],
    target_env: &crate::interpreter::environment::Environment,
) -> Result<RValue, RError> {
    use crate::parser::ast::Expr;

    let list = match args_val {
        RValue::List(l) => l,
        _ => return Ok(RValue::Null),
    };

    // First element is the expression to run on exit
    let expr = list.values.first().map(|(_, v)| v);
    // Second element or named "add" is whether to add (vs replace)
    let add = list
        .values
        .iter()
        .find(|(k, _)| k.as_deref() == Some("add"))
        .or_else(|| list.values.get(1).filter(|(k, _)| k.is_none()))
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .or_else(|| {
            named
                .iter()
                .find(|(n, _)| n == "add")
                .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        })
        .unwrap_or(false);

    if let Some(expr_val) = expr {
        // Convert the RValue (Language/call) back to an Expr
        let expr_ast = match expr_val {
            RValue::Language(lang) => (*lang.inner).clone(),
            _ => Expr::Null,
        };
        target_env.push_on_exit(expr_ast, add, true);
    } else {
        target_env.take_on_exit();
    }

    Ok(RValue::Null)
}

/// Create a vectorized version of a function.
///
/// Returns a new closure that calls `mapply(FUN, ...)` under the hood,
/// so scalar user-defined functions work element-wise on vector inputs.
///
/// @param FUN function to vectorize
/// @param vectorize.args character vector of argument names to vectorize over (default: all formals)
/// @param SIMPLIFY if TRUE (default), simplify the result
/// @return a new function that applies FUN element-wise
#[interpreter_builtin(name = "Vectorize", min_args = 1)]
fn interp_vectorize(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let fun = match_fun(
        positional.first().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let simplify = named
        .iter()
        .find(|(n, _)| n == "SIMPLIFY")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    // Build a closure: function(...) mapply(FUN, ..., SIMPLIFY = <simplify>)
    // The FUN value is captured in the closure's environment.
    let closure_env = Environment::new_child(env);
    closure_env.set(".VEC_FUN".to_string(), fun);
    closure_env.set(
        ".VEC_SIMPLIFY".to_string(),
        RValue::vec(Vector::Logical(vec![Some(simplify)].into())),
    );

    let body = Expr::Call {
        func: Box::new(Expr::Symbol("mapply".to_string())),
        span: None,
        args: vec![
            // FUN as first positional arg (mapply takes FUN as positional[0])
            Arg {
                name: None,
                value: Some(Expr::Symbol(".VEC_FUN".to_string())),
            },
            Arg {
                name: None,
                value: Some(Expr::Dots),
            },
            Arg {
                name: Some("SIMPLIFY".to_string()),
                value: Some(Expr::Symbol(".VEC_SIMPLIFY".to_string())),
            },
        ],
    };

    let params = vec![Param {
        name: "...".to_string(),
        default: None,
        is_dots: true,
    }];

    Ok(RValue::Function(RFunction::Closure {
        params,
        body,
        env: closure_env,
    }))
}

/// Reduce a vector or list to a single value by applying a binary function.
///
/// @param f binary function taking two arguments
/// @param x vector or list to reduce
/// @param init optional initial value for the accumulator
/// @param accumulate if TRUE, return all intermediate results
/// @return the final accumulated value, or a list of intermediate values if accumulate=TRUE
#[interpreter_builtin(name = "Reduce", min_args = 2)]
fn interp_reduce(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Reduce requires at least 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (_fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];
    let init = positional
        .get(2)
        .or_else(|| named.iter().find(|(n, _)| n == "init").map(|(_, v)| v));
    let accumulate = named
        .iter()
        .find(|(n, _)| n == "accumulate")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    if items.is_empty() {
        return Ok(init.cloned().unwrap_or(RValue::Null));
    }

    let (mut acc, start) = match init {
        Some(v) => (v.clone(), 0),
        None => (items[0].clone(), 1),
    };

    let mut accum_results = if accumulate {
        vec![acc.clone()]
    } else {
        vec![]
    };

    // Reduce is inherently sequential — each step depends on the previous.
    // fail_fast has no meaningful "collect errors" behavior here; errors always propagate.
    let env = context.env();
    context.with_interpreter(|interp| {
        for item in items.iter().skip(start) {
            acc = interp.call_function(&f, &[acc, item.clone()], &[], env)?;
            if accumulate {
                accum_results.push(acc.clone());
            }
        }

        if accumulate {
            let values: Vec<(Option<String>, RValue)> =
                accum_results.into_iter().map(|v| (None, v)).collect();
            Ok(RValue::List(RList::new(values)))
        } else {
            Ok(acc)
        }
    })
}

/// Select elements of a vector or list for which a predicate returns TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to filter
/// @return elements of x for which f returns TRUE
#[interpreter_builtin(name = "Filter", min_args = 2)]
fn interp_filter(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Filter requires 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let items: Vec<RValue> = rvalue_to_items(x);

    let mut results = Vec::new();
    context.with_interpreter(|interp| {
        for item in &items {
            if fail_fast {
                let keep = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
                if keep
                    .as_vector()
                    .and_then(|v| v.as_logical_scalar())
                    .unwrap_or(false)
                {
                    results.push(item.clone());
                }
            } else if let Ok(keep) = interp.call_function(&f, std::slice::from_ref(item), &[], env)
            {
                if keep
                    .as_vector()
                    .and_then(|v| v.as_logical_scalar())
                    .unwrap_or(false)
                {
                    results.push(item.clone());
                }
                // Errors are silently skipped (element excluded from results)
            }
        }
        Ok::<(), RError>(())
    })?;

    match x {
        RValue::List(_) => {
            let values: Vec<(Option<String>, RValue)> =
                results.into_iter().map(|v| (None, v)).collect();
            Ok(RValue::List(RList::new(values)))
        }
        _ => {
            if results.is_empty() {
                Ok(RValue::Null)
            } else {
                crate::interpreter::builtins::builtin_c(&results, &[])
            }
        }
    }
}

/// Apply a function to corresponding elements of multiple vectors or lists.
///
/// @param f function to apply
/// @param ... vectors or lists to map over in parallel
/// @return list of results
#[interpreter_builtin(name = "Map", min_args = 2)]
fn interp_map(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Map requires at least 2 arguments".to_string(),
        ));
    }
    let env = context.env();
    let (fail_fast, _extra_named) = extract_fail_fast(named);
    let f = match_fun(&positional[0], env)?;

    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut results = Vec::new();

    context.with_interpreter(|interp| {
        for i in 0..max_len {
            let call_args: Vec<RValue> = seqs
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        RValue::Null
                    } else {
                        s[i % s.len()].clone()
                    }
                })
                .collect();
            let result = if fail_fast {
                interp.call_function(&f, &call_args, &[], env)?
            } else {
                interp
                    .call_function(&f, &call_args, &[], env)
                    .unwrap_or(RValue::Null)
            };
            results.push((None, result));
        }
        Ok::<(), RError>(())
    })?;

    Ok(RValue::List(RList::new(results)))
}

// switch() moved to pre_eval.rs — must not eagerly evaluate all branches

/// Look up a variable by name in an environment.
///
/// @param x character string giving the variable name
/// @param envir environment in which to look up the variable (default: calling environment)
/// @return the value bound to the name
#[interpreter_builtin(name = "get", min_args = 1)]
fn interp_get(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let name = call_args.string("x", 0)?;
    let target_env = call_args.environment_or("envir", usize::MAX, env)?;

    // Check for active bindings — these re-evaluate a function on every access
    if let Some(fun) = target_env.get_active_binding(&name) {
        return context
            .with_interpreter(|interp| interp.call_function(&fun, &[], &[], &target_env))
            .map_err(|flow| match flow {
                RFlow::Error(e) => e,
                other => RError::other(format!("{:?}", other)),
            });
    }

    target_env
        .get(&name)
        .ok_or_else(|| RError::other(format!("object '{}' not found", name)))
}

/// Like `get()` but returns a default value instead of erroring when not found.
///
/// @param x character string giving the variable name
/// @param envir environment to look in (default: calling environment)
/// @param ifnotfound value to return if `x` is not found (default: NULL)
/// @return the value of the variable, or `ifnotfound` if not present
#[interpreter_builtin(name = "get0", min_args = 1)]
fn interp_get0(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let name = call_args.string("x", 0)?;
    let target_env = call_args.environment_or("envir", usize::MAX, env)?;
    let ifnotfound = call_args.value("ifnotfound", usize::MAX).cloned();

    if let Some(fun) = target_env.get_active_binding(&name) {
        return context
            .with_interpreter(|interp| interp.call_function(&fun, &[], &[], &target_env))
            .map_err(|flow| match flow {
                RFlow::Error(e) => e,
                other => RError::other(format!("{:?}", other)),
            });
    }

    Ok(target_env
        .get(&name)
        .unwrap_or_else(|| ifnotfound.unwrap_or(RValue::Null)))
}

/// Assign a value to a variable name in an environment.
///
/// @param x character string giving the variable name
/// @param value the value to assign
/// @param envir environment in which to make the assignment (default: calling environment)
/// @return the assigned value, invisibly
#[interpreter_builtin(name = "assign", min_args = 2)]
fn interp_assign(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let name = call_args.string("x", 0)?;
    let value = call_args.value("value", 1).cloned().unwrap_or(RValue::Null);
    let target_env = call_args.environment_or("envir", usize::MAX, env)?;
    target_env.set(name, value.clone());
    Ok(value)
}

/// Test whether a variable exists in an environment.
///
/// @param x character string giving the variable name
/// @param envir environment to search in (default: calling environment)
/// @return TRUE if the variable exists, FALSE otherwise
#[interpreter_builtin(name = "exists", min_args = 1)]
fn interp_exists(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, _named);
    let name = call_args.optional_string("x", 0).unwrap_or_default();
    let found = call_args
        .environment_or("envir", usize::MAX, env)?
        .get(&name)
        .is_some();
    Ok(RValue::vec(Vector::Logical(vec![Some(found)].into())))
}

/// Read and evaluate an R source file.
///
/// @param file path to the R source file
/// @return the result of evaluating the last expression in the file
#[interpreter_builtin(name = "source", min_args = 1)]
fn interp_source(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'file' argument".to_string()))?;
    let resolved_path = context.interpreter().resolve_path(&path);
    let display_path = resolved_path.to_string_lossy().to_string();
    let source = match std::fs::read_to_string(&resolved_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let bytes = std::fs::read(&resolved_path).map_err(|e2| {
                RError::other(format!("cannot open file '{}': {}", display_path, e2))
            })?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(e) => {
            return Err(RError::other(format!(
                "cannot open file '{}': {}",
                display_path, e
            )))
        }
    };
    let ast = crate::parser::parse_program(&source)
        .map_err(|e| RError::other(format!("parse error in '{}': {}", display_path, e)))?;
    context.with_interpreter(|interp| {
        interp
            .source_stack
            .borrow_mut()
            .push((display_path.clone(), source));
        let result = interp.eval(&ast).map_err(RError::from);
        interp.source_stack.borrow_mut().pop();
        result
    })
}

/// Read and evaluate an R source file in a specified environment.
///
/// Like `source()`, but evaluates the expressions in the given environment
/// rather than the global environment. This is useful for loading code into
/// a specific namespace or local environment.
///
/// @param file path to the R source file
/// @param envir environment in which to evaluate (default: base environment)
/// @return the result of evaluating the last expression in the file (invisibly)
#[interpreter_builtin(name = "sys.source", min_args = 1)]
fn interp_sys_source(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'file' argument".to_string()))?;
    let resolved_path = context.interpreter().resolve_path(&path);
    let display_path = resolved_path.to_string_lossy().to_string();

    // Get environment from named 'envir' argument or second positional
    let env = named
        .iter()
        .find(|(n, _)| n == "envir")
        .map(|(_, v)| v)
        .or_else(|| positional.get(1));

    let source = match std::fs::read_to_string(&resolved_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            let bytes = std::fs::read(&resolved_path).map_err(|e2| {
                RError::other(format!("cannot open file '{}': {}", display_path, e2))
            })?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Err(e) => {
            return Err(RError::other(format!(
                "cannot open file '{}': {}",
                display_path, e
            )))
        }
    };

    let ast = crate::parser::parse_program(&source)
        .map_err(|e| RError::other(format!("parse error in '{}': {}", display_path, e)))?;

    context.with_interpreter(|interp| {
        interp
            .source_stack
            .borrow_mut()
            .push((display_path.clone(), source));
        let result = match env {
            Some(RValue::Environment(target_env)) => {
                interp.eval_in(&ast, target_env).map_err(RError::from)
            }
            _ => interp.eval(&ast).map_err(RError::from),
        };
        interp.source_stack.borrow_mut().pop();
        result
    })
}

// system.time() is in pre_eval.rs — it must time unevaluated expressions

// --- Operator builtins: R operators as first-class functions ---
// These allow `Reduce("+", 1:10)`, `sapply(x, "-")`, `do.call("*", list(3,4))`, etc.

fn eval_binop(op: BinaryOp, args: &[RValue], context: &BuiltinContext) -> Result<RValue, RError> {
    let left = args.first().cloned().unwrap_or(RValue::Null);
    let right = args.get(1).cloned().unwrap_or(RValue::Null);
    context
        .with_interpreter(|interp| interp.eval_binary(op, &left, &right))
        .map_err(RError::from)
}

/// Addition operator as a function (unary positive or binary addition).
///
/// @param e1 first operand (or sole operand for unary +)
/// @param e2 second operand (optional)
/// @return sum of e1 and e2, or e1 unchanged for unary +
#[interpreter_builtin(name = "+", min_args = 1)]
fn interp_op_add(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        context
            .with_interpreter(|interp| interp.eval_unary(UnaryOp::Pos, &args[0]))
            .map_err(RError::from)
    } else {
        eval_binop(BinaryOp::Add, args, context)
    }
}

/// Subtraction operator as a function (unary negation or binary subtraction).
///
/// @param e1 first operand (or sole operand for unary -)
/// @param e2 second operand (optional)
/// @return difference of e1 and e2, or negation of e1 for unary -
#[interpreter_builtin(name = "-", min_args = 1)]
fn interp_op_sub(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if args.len() == 1 {
        context
            .with_interpreter(|interp| interp.eval_unary(UnaryOp::Neg, &args[0]))
            .map_err(RError::from)
    } else {
        eval_binop(BinaryOp::Sub, args, context)
    }
}

/// Multiplication operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return product of e1 and e2
#[interpreter_builtin(name = "*", min_args = 2)]
fn interp_op_mul(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mul, args, context)
}

/// Division operator as a function.
///
/// @param e1 numerator
/// @param e2 denominator
/// @return quotient of e1 and e2
#[interpreter_builtin(name = "/", min_args = 2)]
fn interp_op_div(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Div, args, context)
}

/// Exponentiation operator as a function.
///
/// @param e1 base
/// @param e2 exponent
/// @return e1 raised to the power of e2
#[interpreter_builtin(name = "^", min_args = 2)]
fn interp_op_pow(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Pow, args, context)
}

/// Modulo operator as a function.
///
/// @param e1 dividend
/// @param e2 divisor
/// @return remainder of e1 divided by e2
#[interpreter_builtin(name = "%%", min_args = 2)]
fn interp_op_mod(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Mod, args, context)
}

/// Integer division operator as a function.
///
/// @param e1 dividend
/// @param e2 divisor
/// @return integer quotient of e1 divided by e2
#[interpreter_builtin(name = "%/%", min_args = 2)]
fn interp_op_intdiv(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::IntDiv, args, context)
}

/// Equality comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise equality
#[interpreter_builtin(name = "==", min_args = 2)]
fn interp_op_eq(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Eq, args, context)
}

/// Inequality comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise inequality
#[interpreter_builtin(name = "!=", min_args = 2)]
fn interp_op_ne(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ne, args, context)
}

/// Less-than comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise less-than
#[interpreter_builtin(name = "<", min_args = 2)]
fn interp_op_lt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Lt, args, context)
}

/// Greater-than comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise greater-than
#[interpreter_builtin(name = ">", min_args = 2)]
fn interp_op_gt(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Gt, args, context)
}

/// Less-than-or-equal comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise less-than-or-equal
#[interpreter_builtin(name = "<=", min_args = 2)]
fn interp_op_le(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Le, args, context)
}

/// Greater-than-or-equal comparison operator as a function.
///
/// @param e1 first operand
/// @param e2 second operand
/// @return logical vector indicating element-wise greater-than-or-equal
#[interpreter_builtin(name = ">=", min_args = 2)]
fn interp_op_ge(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Ge, args, context)
}

/// Element-wise logical AND operator as a function.
///
/// @param e1 first logical operand
/// @param e2 second logical operand
/// @return logical vector of element-wise AND results
#[interpreter_builtin(name = "&", min_args = 2)]
fn interp_op_and(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::And, args, context)
}

/// Element-wise logical OR operator as a function.
///
/// @param e1 first logical operand
/// @param e2 second logical operand
/// @return logical vector of element-wise OR results
#[interpreter_builtin(name = "|", min_args = 2)]
fn interp_op_or(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    eval_binop(BinaryOp::Or, args, context)
}

/// Logical NOT operator as a function.
///
/// @param x logical operand
/// @return logical vector of negated values
#[interpreter_builtin(name = "!", min_args = 1)]
fn interp_op_not(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context
        .with_interpreter(|interp| interp.eval_unary(UnaryOp::Not, &args[0]))
        .map_err(RError::from)
}

/// Convert an RValue to a Vec of individual items (for apply/map/filter/reduce).
fn rvalue_to_items(x: &RValue) -> Vec<RValue> {
    match x {
        RValue::Vector(v) => match &v.inner {
            Vector::Raw(vals) => vals
                .iter()
                .map(|&x| RValue::vec(Vector::Raw(vec![x])))
                .collect(),
            Vector::Double(vals) => vals
                .iter_opt()
                .map(|x| RValue::vec(Vector::Double(vec![x].into())))
                .collect(),
            Vector::Integer(vals) => vals
                .iter_opt()
                .map(|x| RValue::vec(Vector::Integer(vec![x].into())))
                .collect(),
            Vector::Complex(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Complex(vec![*x].into())))
                .collect(),
            Vector::Character(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Character(vec![x.clone()].into())))
                .collect(),
            Vector::Logical(vals) => vals
                .iter()
                .map(|x| RValue::vec(Vector::Logical(vec![*x].into())))
                .collect(),
        },
        RValue::List(l) => l.values.iter().map(|(_, v)| v.clone()).collect(),
        _ => vec![x.clone()],
    }
}

/// Invoke the next method in an S3 method dispatch chain.
///
/// @param generic character string naming the generic (optional, inferred from context)
/// @param object the object being dispatched on (optional, inferred from context)
/// @return the result of calling the next method
#[interpreter_builtin(name = "NextMethod")]
fn interp_next_method(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    context
        .with_interpreter(|interp| interp.dispatch_next_method(positional, named, env))
        .map_err(RError::from)
}

/// Get or query the environment of a function.
///
/// @param fun function whose environment to return (optional; returns calling env if omitted)
/// @return the environment of fun, or the calling environment if no argument given
#[interpreter_builtin(name = "environment")]
fn interp_environment(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    match positional.first() {
        Some(RValue::Function(RFunction::Closure { env, .. })) => {
            Ok(RValue::Environment(env.clone()))
        }
        Some(_) => Ok(RValue::Null),
        // No args: return the current (calling) environment
        None => Ok(RValue::Environment(context.env().clone())),
    }
}

/// Coerce a value to an environment.
///
/// @param x integer (search path position), string (environment name), or environment
/// @return the corresponding environment
#[interpreter_builtin(name = "as.environment", min_args = 1)]
fn interp_as_environment(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    match x {
        RValue::Environment(_) => Ok(x.clone()),
        RValue::Vector(rv) => {
            if let Some(n) = rv.as_integer_scalar() {
                return context.with_interpreter(|interp| {
                    match n {
                    1 => Ok(RValue::Environment(interp.global_env.clone())),
                    -1 => {
                        let base = interp
                            .global_env
                            .parent()
                            .unwrap_or_else(|| interp.global_env.clone());
                        Ok(RValue::Environment(base))
                    }
                    _ => Err(RError::new(RErrorKind::Argument, format!(
                        "as.environment({}): only search path positions 1 (global) and -1 (base) are currently supported",
                        n
                    ))),
                }
                });
            }
            if let Some(s) = rv.as_character_scalar() {
                return context.with_interpreter(|interp| match s.as_str() {
                    ".GlobalEnv" | "R_GlobalEnv" => {
                        Ok(RValue::Environment(interp.global_env.clone()))
                    }
                    "package:base" => {
                        let base = interp
                            .global_env
                            .parent()
                            .unwrap_or_else(|| interp.global_env.clone());
                        Ok(RValue::Environment(base))
                    }
                    _ => Err(RError::new(
                        RErrorKind::Argument,
                        format!(
                        "no environment called '{}' was found. Use '.GlobalEnv' or 'package:base'",
                        s
                    ),
                    )),
                });
            }
            Err(RError::new(
                RErrorKind::Argument,
                format!(
                "cannot coerce {} to an environment — expected a number, string, or environment",
                x.type_name()
            ),
            ))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "cannot coerce {} to an environment — expected a number, string, or environment",
                x.type_name()
            ),
        )),
    }
}

/// Return the global environment.
///
/// @return the global environment
#[interpreter_builtin(name = "globalenv", max_args = 0)]
fn interp_globalenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| Ok(RValue::Environment(interp.global_env.clone())))
}

/// Return the base environment.
///
/// @return the base environment (parent of the global environment)
#[interpreter_builtin(name = "baseenv", max_args = 0)]
fn interp_baseenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        Ok(RValue::Environment(
            interp
                .global_env
                .parent()
                .unwrap_or_else(|| interp.global_env.clone()),
        ))
    })
}

/// Return the empty environment (has no parent and no bindings).
///
/// @return the empty environment
#[interpreter_builtin(name = "emptyenv", max_args = 0)]
fn interp_emptyenv(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    Ok(RValue::Environment(Environment::new_empty()))
}

/// Get the call expression of a frame on the call stack.
///
/// @param which frame number (0 = current, positive = counting from bottom)
/// @return the call as a language object, or NULL
#[interpreter_builtin(name = "sys.call", max_args = 1)]
fn interp_sys_call(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return Ok(language_or_null(interp.current_call_expr()));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        let frame = interp
            .call_frame(which)
            .ok_or_else(|| RError::other("not that many frames on the stack"))?;
        Ok(language_or_null(frame.call))
    })
}

/// Get the function of a frame on the call stack.
///
/// @param which frame number (0 = current, positive = counting from bottom)
/// @return the function object for the given frame
#[interpreter_builtin(name = "sys.function", max_args = 1)]
fn interp_sys_function(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return interp
                .current_call_frame()
                .map(|frame| frame.function)
                .ok_or_else(|| RError::other("not that many frames on the stack"));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        interp
            .call_frame(which)
            .map(|frame| frame.function)
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

/// Get the environment of a frame on the call stack.
///
/// @param which frame number (0 = global env, positive = counting from bottom)
/// @return the environment for the given frame
#[interpreter_builtin(name = "sys.frame", max_args = 1)]
fn interp_sys_frame(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let which = optional_frame_index(positional, 0)?;
    context.with_interpreter(|interp| {
        if which == 0 {
            return Ok(RValue::Environment(interp.global_env.clone()));
        }

        if which < 0 {
            return Err(RError::other(
                "negative frame indices are not yet supported",
            ));
        }

        let which = usize::try_from(which).map_err(RError::from)?;
        interp
            .call_frame(which)
            .map(|frame| RValue::Environment(frame.env))
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

/// Get the list of all calls on the call stack.
///
/// @return list of call language objects for all active frames
#[interpreter_builtin(name = "sys.calls", max_args = 0)]
fn interp_sys_calls(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let values = interp
            .call_frames()
            .into_iter()
            .map(|frame| (None, language_or_null(frame.call)))
            .collect();
        Ok(RValue::List(RList::new(values)))
    })
}

/// Get the list of all environments on the call stack.
///
/// @return list of environments for all active frames
#[interpreter_builtin(name = "sys.frames", max_args = 0)]
fn interp_sys_frames(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let values = interp
            .call_frames()
            .into_iter()
            .map(|frame| (None, RValue::Environment(frame.env)))
            .collect();
        Ok(RValue::List(RList::new(values)))
    })
}

/// Get the parent frame indices for all frames on the call stack.
///
/// @return integer vector of parent frame indices
#[interpreter_builtin(name = "sys.parents", max_args = 0)]
fn interp_sys_parents(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let len = interp.call_frames().len();
        let parents: Vec<Option<i64>> = (0..len)
            .map(|i| i64::try_from(i).map(Some))
            .collect::<Result<_, _>>()
            .map_err(RError::from)?;
        Ok(RValue::vec(Vector::Integer(parents.into())))
    })
}

/// Get the on.exit expression for the current frame.
///
/// @return the on.exit expression as a language object, or NULL if none
#[interpreter_builtin(name = "sys.on.exit", max_args = 0)]
fn interp_sys_on_exit(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let frame = match interp.current_call_frame() {
            Some(frame) => frame,
            None => return Ok(RValue::Null),
        };

        let exprs = frame.env.peek_on_exit();
        match exprs.len() {
            0 => Ok(RValue::Null),
            1 => Ok(RValue::Language(Language::new(exprs[0].clone()))),
            _ => Ok(RValue::Language(Language::new(
                crate::parser::ast::Expr::Block(exprs),
            ))),
        }
    })
}

/// Get the number of frames on the call stack.
///
/// @return integer giving the current stack depth
#[interpreter_builtin(name = "sys.nframe", max_args = 0)]
fn interp_sys_nframe(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let len = i64::try_from(interp.call_frames().len()).map_err(RError::from)?;
        Ok(RValue::vec(Vector::Integer(vec![Some(len)].into())))
    })
}

/// Get the number of arguments supplied to the current function call.
///
/// @return integer giving the number of supplied arguments
#[interpreter_builtin(name = "nargs", max_args = 0)]
fn interp_nargs(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let count = interp
            .current_call_frame()
            .map(|frame| frame.supplied_arg_count)
            .unwrap_or(0);
        Ok(RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(count).map_err(RError::from)?)].into(),
        )))
    })
}

/// Recursively call the current function with new arguments.
///
/// `Recall(...)` finds the currently executing user function from the call stack
/// and calls it again with the supplied arguments. This is useful for anonymous
/// recursive functions that don't have a name to call themselves by.
///
/// @param ... arguments to pass to the recursive call
/// @return the result of calling the current function with the new arguments
#[interpreter_builtin(name = "Recall")]
fn interp_recall(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    context.with_interpreter(|interp| {
        let frame = interp.current_call_frame().ok_or_else(|| {
            RError::other(
                "Recall() called from outside a function. \
                 Recall() can only be used inside a function body to recursively \
                 call the current function.",
            )
        })?;
        interp
            .call_function(&frame.function, positional, named, env)
            .map_err(RError::from)
    })
}

/// Get the environment of the parent (calling) frame.
///
/// @param n number of generations to go back (default 1)
/// @return the environment of the n-th parent frame
#[interpreter_builtin(name = "parent.frame", max_args = 1)]
fn interp_parent_frame(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let n = optional_frame_index(positional, 1)?;
    if n <= 0 {
        return Err(RError::new(
            RErrorKind::Argument,
            "invalid 'n' value".to_string(),
        ));
    }

    context.with_interpreter(|interp| {
        let depth = interp.call_frames().len();
        let n = usize::try_from(n).map_err(RError::from)?;
        if n >= depth {
            return Ok(RValue::Environment(interp.global_env.clone()));
        }

        let target = depth - n;
        interp
            .call_frame(target)
            .map(|frame| RValue::Environment(frame.env))
            .ok_or_else(|| RError::other("not that many frames on the stack"))
    })
}

/// List the names of objects in an environment.
///
/// @param envir environment to list (default: calling environment)
/// @return character vector of variable names
#[interpreter_builtin(name = "ls", names = ["objects"])]
fn interp_ls(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let target_env = CallArgs::new(positional, named).environment_or("envir", 0, env)?;

    let names = target_env.ls();
    let chars: Vec<Option<String>> = names.into_iter().map(Some).collect();
    Ok(RValue::vec(Vector::Character(chars.into())))
}

// rm() / remove() is implemented as a pre_eval builtin in pre_eval.rs
// to support NSE (bare symbol names like `rm(x)` instead of `rm("x")`)

/// Lock an environment so no new bindings can be added.
///
/// @param env environment to lock
/// @param bindings if TRUE, also lock all existing bindings (default FALSE)
/// @return NULL (invisibly)
#[interpreter_builtin(name = "lockEnvironment", min_args = 1)]
fn interp_lock_environment(
    positional: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = match positional.first() {
        Some(RValue::Environment(e)) => e.clone(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };

    let call_args = CallArgs::new(positional, named);
    let bindings = call_args.logical_flag("bindings", 1, false);
    env.lock(bindings);
    Ok(RValue::Null)
}

/// Check whether an environment is locked.
///
/// @param env environment to query
/// @return logical scalar: TRUE if locked, FALSE otherwise
#[interpreter_builtin(name = "environmentIsLocked", min_args = 1)]
fn interp_environment_is_locked(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let locked = match positional.first() {
        Some(RValue::Environment(e)) => e.is_locked(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(locked)].into())))
}

/// Lock a specific binding in an environment.
///
/// @param sym name of the binding to lock (character string)
/// @param env environment containing the binding
/// @return NULL (invisibly)
#[interpreter_builtin(name = "lockBinding", min_args = 2)]
fn interp_lock_binding(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let sym = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "not a valid symbol name".to_string()))?;
    let env = match positional.get(1) {
        Some(RValue::Environment(e)) => e.clone(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };
    env.lock_binding(&sym);
    Ok(RValue::Null)
}

/// Check whether a binding is locked in an environment.
///
/// @param sym name of the binding to check (character string)
/// @param env environment containing the binding
/// @return logical scalar: TRUE if the binding is locked, FALSE otherwise
#[interpreter_builtin(name = "bindingIsLocked", min_args = 2)]
fn interp_binding_is_locked(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let sym = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "not a valid symbol name".to_string()))?;
    let locked = match positional.get(1) {
        Some(RValue::Environment(e)) => e.binding_is_locked(&sym),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };
    Ok(RValue::vec(Vector::Logical(vec![Some(locked)].into())))
}

/// Create an active binding.
///
/// Active bindings call a function every time they are accessed.
/// The function `fun` is stored in the environment and re-evaluated
/// on every read of `sym`.
///
/// @param sym name for the binding (character string)
/// @param fun zero-argument function to call on access
/// @param env environment in which to create the binding
/// @return NULL (invisibly)
#[interpreter_builtin(name = "makeActiveBinding", min_args = 3)]
fn interp_make_active_binding(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let sym = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "not a valid symbol name".to_string()))?;
    let fun = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "'fun' argument is missing".to_string(),
        )
    })?;
    let env = match positional.get(2) {
        Some(RValue::Environment(e)) => e.clone(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };

    env.set_active_binding(sym, fun.clone());
    Ok(RValue::Null)
}

/// Check whether a binding is an active binding.
///
/// @param sym name of the binding (character string)
/// @param env environment in which to check
/// @return logical scalar
#[interpreter_builtin(name = "isActiveBinding", min_args = 2)]
fn interp_is_active_binding(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let sym = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "not a valid symbol name".to_string()))?;
    let env = match positional.get(1) {
        Some(RValue::Environment(e)) => e.clone(),
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "not an environment".to_string(),
            ))
        }
    };

    let is_active = env.is_local_active_binding(&sym);
    Ok(RValue::vec(Vector::Logical(vec![Some(is_active)].into())))
}

/// Evaluate an expression in a specified environment.
///
/// @param expr expression to evaluate (language object or character string)
/// @param envir environment in which to evaluate (default: calling environment)
/// @return the result of evaluating expr
#[interpreter_builtin(name = "eval", min_args = 1)]
fn interp_eval(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let call_args = CallArgs::new(positional, named);
    let expr = positional.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'expr' is missing".to_string(),
        )
    })?;

    let eval_env = call_args.environment_or("envir", 1, env)?;

    match expr {
        // Language object: evaluate the AST
        RValue::Language(ast) => context
            .with_interpreter(|interp| interp.eval_in(ast, &eval_env))
            .map_err(RError::from),
        // Character string: parse then eval
        RValue::Vector(rv) if matches!(rv.inner, Vector::Character(_)) => {
            let text = rv.as_character_scalar().unwrap_or_default();
            let parsed = crate::parser::parse_program(&text)
                .map_err(|e| RError::new(RErrorKind::Parse, format!("{}", e)))?;
            context
                .with_interpreter(|interp| interp.eval_in(&parsed, &eval_env))
                .map_err(RError::from)
        }
        // Already evaluated value: return as-is
        _ => Ok(expr.clone()),
    }
}

/// Parse R source text into a language object.
///
/// @param text character string containing R code to parse
/// @return a language object representing the parsed expression
#[interpreter_builtin(name = "parse", min_args = 0)]
fn interp_parse(
    positional: &[RValue],
    named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let text = named
        .iter()
        .find(|(n, _)| n == "text")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            positional
                .first()
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'text' is missing".to_string(),
            )
        })?;

    let parsed = crate::parser::parse_program(&text)
        .map_err(|e| RError::new(RErrorKind::Parse, format!("{}", e)))?;
    Ok(RValue::Language(Language::new(parsed)))
}

// --- apply family: apply, mapply, tapply, by ---

/// Apply a function over rows or columns of a matrix.
///
/// @param X matrix or array
/// @param MARGIN 1 for rows, 2 for columns
/// @param FUN function to apply
/// @param ... additional arguments passed to FUN
/// @return vector, matrix, or list of results
#[interpreter_builtin(name = "apply", min_args = 3)]
fn interp_apply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'X' is missing".to_string()))?;
    let margin_val = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'MARGIN' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let margin = margin_val
        .as_vector()
        .and_then(|v| v.as_integer_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "MARGIN must be 1 (rows) or 2 (columns) — got a non-integer value".to_string(),
            )
        })?;

    // Extract dim attribute — X must be a matrix
    let (nrow, ncol, vec_inner) = match x {
        RValue::Vector(rv) => {
            let dims = super::get_dim_ints(rv.get_attr("dim")).ok_or_else(|| {
                RError::new(
                    RErrorKind::Argument,
                    "X must have a 'dim' attribute (i.e. be a matrix or array). \
                     Use matrix() to create one."
                        .to_string(),
                )
            })?;
            if dims.len() < 2 {
                return Err(RError::new(
                    RErrorKind::Argument,
                    "X must be a 2D matrix for apply() — got an array with fewer than 2 dimensions"
                        .to_string(),
                ));
            }
            let nr = usize::try_from(dims[0].unwrap_or(0))?;
            let nc = usize::try_from(dims[1].unwrap_or(0))?;
            (nr, nc, &rv.inner)
        }
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "apply() requires a matrix (vector with dim attribute) as the first argument"
                    .to_string(),
            ))
        }
    };

    // Extra args to pass to FUN (positional args beyond the first 3)
    let extra_args: Vec<RValue> = positional.iter().skip(3).cloned().collect();

    match margin {
        1 => {
            // Apply FUN to each row — extract row indices preserving original type
            let mut results: Vec<RValue> = Vec::with_capacity(nrow);
            context.with_interpreter(|interp| {
                for i in 0..nrow {
                    // Column-major: element (i, j) is at index i + j * nrow
                    let indices: Vec<usize> = (0..ncol).map(|j| i + j * nrow).collect();
                    let row_vec = vec_inner.select_indices(&indices);
                    let row_val = RValue::vec(row_vec);
                    let mut call_args = vec![row_val];
                    call_args.extend(extra_args.iter().cloned());
                    if fail_fast {
                        let result = interp.call_function(&fun, &call_args, &extra_named, env)?;
                        results.push(result);
                    } else {
                        match interp.call_function(&fun, &call_args, &extra_named, env) {
                            Ok(result) => results.push(result),
                            Err(_) => results.push(RValue::Null),
                        }
                    }
                }
                Ok::<(), RError>(())
            })?;
            simplify_apply_results(results)
        }
        2 => {
            // Apply FUN to each column — extract column indices preserving original type
            let mut results: Vec<RValue> = Vec::with_capacity(ncol);
            context.with_interpreter(|interp| {
                for j in 0..ncol {
                    // Column-major: column j starts at j * nrow
                    let indices: Vec<usize> = (0..nrow).map(|i| i + j * nrow).collect();
                    let col_vec = vec_inner.select_indices(&indices);
                    let col_val = RValue::vec(col_vec);
                    let mut call_args = vec![col_val];
                    call_args.extend(extra_args.iter().cloned());
                    if fail_fast {
                        let result = interp.call_function(&fun, &call_args, &extra_named, env)?;
                        results.push(result);
                    } else {
                        match interp.call_function(&fun, &call_args, &extra_named, env) {
                            Ok(result) => results.push(result),
                            Err(_) => results.push(RValue::Null),
                        }
                    }
                }
                Ok::<(), RError>(())
            })?;
            simplify_apply_results(results)
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            format!(
                "MARGIN must be 1 (rows) or 2 (columns) — got {}. \
             Higher-dimensional margins are not yet supported.",
                margin
            ),
        )),
    }
}

/// Simplify apply() results: if all results are scalars, return a vector;
/// if all are equal-length vectors, return a matrix; otherwise return a list.
fn simplify_apply_results(results: Vec<RValue>) -> Result<RValue, RError> {
    if results.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Check if all results are scalar
    let all_scalar = results.iter().all(|r| r.length() == 1);
    if all_scalar {
        let first_type = results[0].type_name();
        let all_same = results.iter().all(|r| r.type_name() == first_type);
        if all_same {
            match first_type {
                "double" => {
                    let vals: Vec<Option<f64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Double(vals.into())));
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Integer(vals.into())));
                }
                "character" => {
                    let vals: Vec<Option<String>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Character(vals.into())));
                }
                "logical" => {
                    let vals: Vec<Option<bool>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return Ok(RValue::vec(Vector::Logical(vals.into())));
                }
                _ => {}
            }
        }
    }

    // Check if all results are equal-length vectors — return a matrix
    let first_len = results[0].length();
    let all_same_len = first_len > 1 && results.iter().all(|r| r.length() == first_len);
    if all_same_len {
        // Build a matrix: each result becomes a column (R's apply convention)
        let ncol = results.len();
        let nrow = first_len;
        let mut mat_data: Vec<Option<f64>> = Vec::with_capacity(nrow * ncol);
        for result in &results {
            if let Some(v) = result.as_vector() {
                mat_data.extend(v.to_doubles());
            }
        }
        let mut rv = RVector::from(Vector::Double(mat_data.into()));
        rv.set_attr(
            "class".to_string(),
            RValue::vec(Vector::Character(
                vec![Some("matrix".to_string()), Some("array".to_string())].into(),
            )),
        );
        rv.set_attr(
            "dim".to_string(),
            RValue::vec(Vector::Integer(
                vec![Some(i64::try_from(nrow)?), Some(i64::try_from(ncol)?)].into(),
            )),
        );
        return Ok(RValue::Vector(rv));
    }

    // Fall back to a list
    let values: Vec<(Option<String>, RValue)> = results.into_iter().map(|v| (None, v)).collect();
    Ok(RValue::List(RList::new(values)))
}

/// Apply a function to corresponding elements of multiple vectors.
///
/// @param FUN function to apply
/// @param ... vectors to iterate over in parallel
/// @param MoreArgs list of additional arguments passed to FUN in every call
/// @param SIMPLIFY if TRUE, simplify the result to a vector or matrix
/// @return simplified vector or list of results
#[interpreter_builtin(name = "mapply", min_args = 2)]
fn interp_mapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // mapply(FUN, ..., MoreArgs = NULL, SIMPLIFY = TRUE, USE.NAMES = TRUE)
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let fun = match_fun(
        positional.first().ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let simplify = extra_named
        .iter()
        .find(|(n, _)| n == "SIMPLIFY")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    // Extract MoreArgs: a list of additional arguments to pass to FUN in every call.
    // Named elements become named args; unnamed elements become positional args.
    let (more_positional, more_named): (Vec<RValue>, Vec<(String, RValue)>) = extra_named
        .iter()
        .find(|(n, _)| n == "MoreArgs")
        .map(|(_, v)| match v {
            RValue::List(l) => {
                let mut pos = Vec::new();
                let mut named = Vec::new();
                for (name, val) in &l.values {
                    match name {
                        Some(n) if !n.is_empty() => named.push((n.clone(), val.clone())),
                        _ => pos.push(val.clone()),
                    }
                }
                (pos, named)
            }
            _ => (Vec::new(), Vec::new()),
        })
        .unwrap_or_default();

    // Collect the input sequences (all positional args after FUN, excluding named)
    let seqs: Vec<Vec<RValue>> = positional[1..].iter().map(rvalue_to_items).collect();

    if seqs.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Find the longest sequence for recycling
    let max_len = seqs.iter().map(|s| s.len()).max().unwrap_or(0);

    let mut results: Vec<RValue> = Vec::with_capacity(max_len);

    context.with_interpreter(|interp| {
        for i in 0..max_len {
            let mut call_args: Vec<RValue> = seqs
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        RValue::Null
                    } else {
                        s[i % s.len()].clone()
                    }
                })
                .collect();
            // Append MoreArgs positional values
            call_args.extend(more_positional.iter().cloned());
            let result = if fail_fast {
                interp.call_function(&fun, &call_args, &more_named, env)?
            } else {
                interp
                    .call_function(&fun, &call_args, &more_named, env)
                    .unwrap_or(RValue::Null)
            };
            results.push(result);
        }
        Ok::<(), RError>(())
    })?;

    if simplify {
        let all_scalar = results.iter().all(|r| r.length() == 1);
        if all_scalar && !results.is_empty() {
            let first_type = results[0].type_name();
            let all_same = results.iter().all(|r| r.type_name() == first_type);
            if all_same {
                match first_type {
                    "double" => {
                        let vals: Vec<Option<f64>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Double(vals.into())));
                    }
                    "integer" => {
                        let vals: Vec<Option<i64>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Integer(vals.into())));
                    }
                    "character" => {
                        let vals: Vec<Option<String>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Character(vals.into())));
                    }
                    "logical" => {
                        let vals: Vec<Option<bool>> = results
                            .iter()
                            .filter_map(|r| {
                                r.as_vector()
                                    .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                            })
                            .collect();
                        return Ok(RValue::vec(Vector::Logical(vals.into())));
                    }
                    _ => {}
                }
            }
        }
    }

    let values: Vec<(Option<String>, RValue)> = results.into_iter().map(|v| (None, v)).collect();
    Ok(RValue::List(RList::new(values)))
}

/// Apply a function to groups of values defined by a factor/index.
///
/// @param X vector of values to split into groups
/// @param INDEX factor or vector defining the groups
/// @param FUN function to apply to each group
/// @return named vector or list of per-group results
#[interpreter_builtin(name = "tapply", min_args = 3)]
fn interp_tapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // tapply(X, INDEX, FUN)
    let (fail_fast, extra_named) = extract_fail_fast(named);
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'X' is missing".to_string()))?;
    let index = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'INDEX' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    let x_items = rvalue_to_items(x);
    let index_items = rvalue_to_items(index);

    if x_items.len() != index_items.len() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "arguments 'X' (length {}) and 'INDEX' (length {}) must have the same length",
                x_items.len(),
                index_items.len()
            ),
        ));
    }

    // Convert index values to string keys for grouping
    let index_keys: Vec<String> = index_items
        .iter()
        .map(|v| match v {
            RValue::Vector(rv) => rv
                .inner
                .as_character_scalar()
                .unwrap_or_else(|| format!("{}", v)),
            _ => format!("{}", v),
        })
        .collect();

    // Collect unique group names preserving first-seen order
    let mut group_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for key in &index_keys {
        if seen.insert(key.clone()) {
            group_names.push(key.clone());
        }
    }

    // Group X values by INDEX
    let mut groups: std::collections::HashMap<String, Vec<RValue>> =
        std::collections::HashMap::new();
    for (item, key) in x_items.into_iter().zip(index_keys.iter()) {
        groups.entry(key.clone()).or_default().push(item);
    }

    // Apply FUN to each group
    let mut result_entries: Vec<(Option<String>, RValue)> = Vec::with_capacity(group_names.len());

    context.with_interpreter(|interp| {
        for name in &group_names {
            let group = groups.remove(name).unwrap_or_default();
            let group_vec = combine_items_to_vector(&group);
            if fail_fast {
                let result = interp.call_function(&fun, &[group_vec], &extra_named, env)?;
                result_entries.push((Some(name.clone()), result));
            } else {
                match interp.call_function(&fun, &[group_vec], &extra_named, env) {
                    Ok(result) => result_entries.push((Some(name.clone()), result)),
                    Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                }
            }
        }
        Ok::<(), RError>(())
    })?;

    // Try to simplify to a named vector if all results are scalar
    let all_scalar = result_entries.iter().all(|(_, v)| v.length() == 1);
    if all_scalar && !result_entries.is_empty() {
        let first_type = result_entries[0].1.type_name();
        let all_same = result_entries
            .iter()
            .all(|(_, v)| v.type_name() == first_type);
        if all_same {
            let names: Vec<Option<String>> =
                result_entries.iter().map(|(n, _)| n.clone()).collect();
            match first_type {
                "double" => {
                    let vals: Vec<Option<f64>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Double(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Integer(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "character" => {
                    let vals: Vec<Option<String>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Character(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                "logical" => {
                    let vals: Vec<Option<bool>> = result_entries
                        .iter()
                        .filter_map(|(_, r)| {
                            r.as_vector()
                                .map(|v| v.to_logicals().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    let mut rv = RVector::from(Vector::Logical(vals.into()));
                    rv.set_attr(
                        "names".to_string(),
                        RValue::vec(Vector::Character(names.into())),
                    );
                    return Ok(RValue::Vector(rv));
                }
                _ => {}
            }
        }
    }

    Ok(RValue::List(RList::new(result_entries)))
}

/// Combine a list of scalar RValues back into a single vector RValue.
fn combine_items_to_vector(items: &[RValue]) -> RValue {
    if items.is_empty() {
        return RValue::Null;
    }

    // Determine the type from the first element
    let first_type = items[0].type_name();
    let all_same = items.iter().all(|v| v.type_name() == first_type);

    if all_same {
        match first_type {
            "double" => {
                let vals: Vec<Option<f64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_doubles())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
            "integer" => {
                let vals: Vec<Option<i64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_integers())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Integer(vals.into()))
            }
            "character" => {
                let vals: Vec<Option<String>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_characters())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Character(vals.into()))
            }
            "logical" => {
                let vals: Vec<Option<bool>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_logicals())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Logical(vals.into()))
            }
            _ => {
                // Fall back to coercing to doubles
                let vals: Vec<Option<f64>> = items
                    .iter()
                    .flat_map(|r| {
                        r.as_vector()
                            .map(|v| v.to_doubles())
                            .unwrap_or_else(|| vec![None])
                    })
                    .collect();
                RValue::vec(Vector::Double(vals.into()))
            }
        }
    } else {
        // Mixed types: coerce to doubles (R's coercion hierarchy)
        let vals: Vec<Option<f64>> = items
            .iter()
            .flat_map(|r| {
                r.as_vector()
                    .map(|v| v.to_doubles())
                    .unwrap_or_else(|| vec![None])
            })
            .collect();
        RValue::vec(Vector::Double(vals.into()))
    }
}

/// Apply a function to subsets of a data frame or vector split by a grouping factor.
///
/// @param data data frame or vector to split
/// @param INDICES factor or vector defining the groups
/// @param FUN function to apply to each subset
/// @return list of per-group results
#[interpreter_builtin(name = "by", min_args = 3)]
fn interp_by(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    // by(data, INDICES, FUN) — similar to tapply but for data-frame-like objects.
    let (fail_fast, extra_named) = extract_fail_fast(named);
    // For vectors, delegate to tapply-like behavior.
    // For lists/data frames, split rows by INDICES and apply FUN to each subset.
    let data = positional.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'data' is missing".to_string(),
        )
    })?;
    let indices = positional.get(1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'INDICES' is missing".to_string(),
        )
    })?;
    let fun = match_fun(
        positional.get(2).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?,
        env,
    )?;

    // For atomic vectors, treat like tapply
    if matches!(data, RValue::Vector(_)) {
        let x_items = rvalue_to_items(data);
        let index_items = rvalue_to_items(indices);

        if x_items.len() != index_items.len() {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                "arguments 'data' (length {}) and 'INDICES' (length {}) must have the same length",
                x_items.len(),
                index_items.len()
            ),
            ));
        }

        let index_keys: Vec<String> = index_items
            .iter()
            .map(|v| match v {
                RValue::Vector(rv) => rv
                    .inner
                    .as_character_scalar()
                    .unwrap_or_else(|| format!("{}", v)),
                _ => format!("{}", v),
            })
            .collect();

        let mut group_names: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in &index_keys {
            if seen.insert(key.clone()) {
                group_names.push(key.clone());
            }
        }

        let mut groups: std::collections::HashMap<String, Vec<RValue>> =
            std::collections::HashMap::new();
        for (item, key) in x_items.into_iter().zip(index_keys.iter()) {
            groups.entry(key.clone()).or_default().push(item);
        }

        let mut result_entries: Vec<(Option<String>, RValue)> =
            Vec::with_capacity(group_names.len());

        context.with_interpreter(|interp| {
            for name in &group_names {
                let group = groups.remove(name).unwrap_or_default();
                let group_vec = combine_items_to_vector(&group);
                if fail_fast {
                    let result = interp.call_function(&fun, &[group_vec], &extra_named, env)?;
                    result_entries.push((Some(name.clone()), result));
                } else {
                    match interp.call_function(&fun, &[group_vec], &extra_named, env) {
                        Ok(result) => result_entries.push((Some(name.clone()), result)),
                        Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                    }
                }
            }
            Ok::<(), RError>(())
        })?;

        return Ok(RValue::List(RList::new(result_entries)));
    }

    // For lists (including data frames), split by INDICES and apply FUN
    if let RValue::List(list) = data {
        let index_items = rvalue_to_items(indices);

        // For a data frame, determine nrow from the first column
        let nrow = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

        if index_items.len() != nrow {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                "arguments 'data' ({} rows) and 'INDICES' (length {}) must have the same length",
                nrow,
                index_items.len()
            ),
            ));
        }

        let index_keys: Vec<String> = index_items
            .iter()
            .map(|v| match v {
                RValue::Vector(rv) => rv
                    .inner
                    .as_character_scalar()
                    .unwrap_or_else(|| format!("{}", v)),
                _ => format!("{}", v),
            })
            .collect();

        let mut group_names: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in &index_keys {
            if seen.insert(key.clone()) {
                group_names.push(key.clone());
            }
        }

        // For each group, build a subset data frame and call FUN
        let mut result_entries: Vec<(Option<String>, RValue)> =
            Vec::with_capacity(group_names.len());

        context.with_interpreter(|interp| {
            for name in &group_names {
                // Find row indices belonging to this group
                let row_indices: Vec<usize> = index_keys
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.as_str() == name)
                    .map(|(i, _)| i)
                    .collect();

                // Build a subset list (data frame) with only these rows
                let mut subset_cols: Vec<(Option<String>, RValue)> = Vec::new();
                for (col_name, col_val) in &list.values {
                    let col_items = rvalue_to_items(col_val);
                    let subset: Vec<RValue> = row_indices
                        .iter()
                        .filter_map(|&i| col_items.get(i).cloned())
                        .collect();
                    let subset_vec = combine_items_to_vector(&subset);
                    subset_cols.push((col_name.clone(), subset_vec));
                }

                let mut subset_list = RList::new(subset_cols);
                // Preserve data.frame class if the original had it
                if let Some(cls) = list.get_attr("class") {
                    subset_list.set_attr("class".to_string(), cls.clone());
                }
                // Set row.names for the subset
                let row_names: Vec<Option<i64>> =
                    (1..=i64::try_from(row_indices.len())?).map(Some).collect();
                subset_list.set_attr(
                    "row.names".to_string(),
                    RValue::vec(Vector::Integer(row_names.into())),
                );
                // Set names attribute
                if let Some(names) = list.get_attr("names") {
                    subset_list.set_attr("names".to_string(), names.clone());
                }

                let subset_val = RValue::List(subset_list);
                if fail_fast {
                    let result = interp.call_function(&fun, &[subset_val], &extra_named, env)?;
                    result_entries.push((Some(name.clone()), result));
                } else {
                    match interp.call_function(&fun, &[subset_val], &extra_named, env) {
                        Ok(result) => result_entries.push((Some(name.clone()), result)),
                        Err(_) => result_entries.push((Some(name.clone()), RValue::Null)),
                    }
                }
            }
            Ok::<(), RError>(())
        })?;

        return Ok(RValue::List(RList::new(result_entries)));
    }

    Err(RError::new(
        RErrorKind::Argument,
        "by() requires a vector, list, or data frame as 'data'".to_string(),
    ))
}

// region: split / unsplit / aggregate

/// Split a vector or data frame into groups defined by a factor.
///
/// @param x vector or data frame to split
/// @param f factor or vector defining the groups (same length as x, or nrow(x) for data frames)
/// @param drop if TRUE, drop unused factor levels (currently ignored)
/// @return named list of subsets
#[interpreter_builtin(min_args = 2)]
fn interp_split(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;
    let f = positional
        .get(1)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'f' is missing".to_string()))?;

    split_impl(x, f)
}

/// Internal split implementation shared by split() and aggregate().
fn split_impl(x: &RValue, f: &RValue) -> Result<RValue, RError> {
    let f_items = rvalue_to_items(f);

    // Convert factor values to string keys
    let f_keys: Vec<String> = f_items
        .iter()
        .map(|v| match v {
            RValue::Vector(rv) => rv
                .inner
                .as_character_scalar()
                .unwrap_or_else(|| format!("{}", v)),
            _ => format!("{}", v),
        })
        .collect();

    // Collect unique group names preserving first-seen order
    let mut group_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for key in &f_keys {
        if seen.insert(key.clone()) {
            group_names.push(key.clone());
        }
    }

    match x {
        RValue::Vector(_) => {
            let x_items = rvalue_to_items(x);
            if x_items.len() != f_keys.len() {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "'x' (length {}) and 'f' (length {}) must have the same length",
                        x_items.len(),
                        f_keys.len()
                    ),
                ));
            }

            let mut groups: std::collections::HashMap<String, Vec<RValue>> =
                std::collections::HashMap::new();
            for (item, key) in x_items.into_iter().zip(f_keys.iter()) {
                groups.entry(key.clone()).or_default().push(item);
            }

            let entries: Vec<(Option<String>, RValue)> = group_names
                .into_iter()
                .map(|name| {
                    let items = groups.remove(&name).unwrap_or_default();
                    let vec = combine_items_to_vector(&items);
                    (Some(name), vec)
                })
                .collect();

            Ok(RValue::List(RList::new(entries)))
        }
        RValue::List(list) => {
            // Data frame: split rows by f
            let nrow = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);
            if f_keys.len() != nrow {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "data frame has {} rows but 'f' has length {}",
                        nrow,
                        f_keys.len()
                    ),
                ));
            }

            let entries: Vec<(Option<String>, RValue)> = group_names
                .into_iter()
                .map(|name| {
                    let row_indices: Vec<usize> = f_keys
                        .iter()
                        .enumerate()
                        .filter(|(_, k)| k.as_str() == name)
                        .map(|(i, _)| i)
                        .collect();

                    let mut subset_cols: Vec<(Option<String>, RValue)> = Vec::new();
                    for (col_name, col_val) in &list.values {
                        let col_items = rvalue_to_items(col_val);
                        let subset: Vec<RValue> = row_indices
                            .iter()
                            .filter_map(|&i| col_items.get(i).cloned())
                            .collect();
                        let subset_vec = combine_items_to_vector(&subset);
                        subset_cols.push((col_name.clone(), subset_vec));
                    }

                    let mut subset_list = RList::new(subset_cols);
                    if let Some(cls) = list.get_attr("class") {
                        subset_list.set_attr("class".to_string(), cls.clone());
                    }
                    if let Some(names) = list.get_attr("names") {
                        subset_list.set_attr("names".to_string(), names.clone());
                    }
                    // row_indices.len() is bounded by original data frame row count
                    let n_rows = i64::try_from(row_indices.len()).unwrap_or(0);
                    let row_names: Vec<Option<i64>> = (1..=n_rows).map(Some).collect();
                    subset_list.set_attr(
                        "row.names".to_string(),
                        RValue::vec(Vector::Integer(row_names.into())),
                    );

                    (Some(name), RValue::List(subset_list))
                })
                .collect();

            Ok(RValue::List(RList::new(entries)))
        }
        _ => Err(RError::new(
            RErrorKind::Argument,
            "split() requires a vector, list, or data frame as 'x'".to_string(),
        )),
    }
}

/// Reverse of split: reassemble a vector from a split list.
///
/// @param value list of vectors (as produced by split())
/// @param f factor or vector defining the groups (same length as the original vector)
/// @return vector with elements placed back at their original positions
#[interpreter_builtin(min_args = 2)]
fn interp_unsplit(
    positional: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let value = positional.first().ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'value' is missing".to_string(),
        )
    })?;
    let f = positional
        .get(1)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'f' is missing".to_string()))?;

    let f_items = rvalue_to_items(f);
    let n = f_items.len();

    // Convert factor values to string keys
    let f_keys: Vec<String> = f_items
        .iter()
        .map(|v| match v {
            RValue::Vector(rv) => rv
                .inner
                .as_character_scalar()
                .unwrap_or_else(|| format!("{}", v)),
            _ => format!("{}", v),
        })
        .collect();

    // value must be a named list
    let list = match value {
        RValue::List(l) => l,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "unsplit() requires a list as 'value'".to_string(),
            ))
        }
    };

    // Build a map from group name to items iterator
    let mut group_items: std::collections::HashMap<String, Vec<RValue>> =
        std::collections::HashMap::new();
    for (name, val) in &list.values {
        if let Some(name) = name {
            group_items.insert(name.clone(), rvalue_to_items(val));
        }
    }

    // Track how many items we've consumed from each group
    let mut group_cursors: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    let mut result: Vec<RValue> = Vec::with_capacity(n);
    for key in &f_keys {
        let cursor = group_cursors.entry(key.clone()).or_insert(0);
        let item = group_items
            .get(key)
            .and_then(|items| items.get(*cursor))
            .cloned()
            .unwrap_or(RValue::Null);
        *cursor += 1;
        result.push(item);
    }

    Ok(combine_items_to_vector(&result))
}

/// Aggregate data by groups, applying a function to each group.
///
/// Supports two calling conventions:
///   aggregate(x, by, FUN) — x is a vector/matrix, by is a list of grouping vectors
///   aggregate(formula, data, FUN) — formula interface (y ~ x, data=df, FUN=mean)
///
/// @param x numeric vector or data frame column to aggregate, or a formula
/// @param by list of grouping vectors (each same length as x), or data frame (formula interface)
/// @param FUN function to apply to each group
/// @param data data frame (named argument, formula interface)
/// @return data frame with grouping columns and aggregated value columns
#[interpreter_builtin(min_args = 2)]
fn interp_aggregate(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let (fail_fast, extra_named) = extract_fail_fast(named);

    let first = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    // Check if first argument is a formula (Language with class "formula")
    let is_formula = match first {
        RValue::Language(lang) => lang
            .get_attr("class")
            .and_then(|v| v.as_vector()?.as_character_scalar())
            .is_some_and(|c| c == "formula"),
        _ => false,
    };

    if is_formula {
        return aggregate_formula(
            first,
            positional,
            named,
            &extra_named,
            fail_fast,
            env,
            context,
        );
    }

    // Standard interface: aggregate(x, by, FUN)
    let by = positional
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "by").map(|(_, v)| v))
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'by' is missing".to_string()))?;
    let fun_val = positional
        .get(2)
        .or_else(|| named.iter().find(|(n, _)| n == "FUN").map(|(_, v)| v))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?;
    let fun = match_fun(fun_val, env)?;

    aggregate_standard(first, by, &fun, &extra_named, fail_fast, env, context)
}

/// Extract column names from a formula expression (e.g., y ~ x parses to lhs=y, rhs=x).
fn extract_formula_vars(expr: &Expr) -> (Vec<String>, Vec<String>) {
    match expr {
        Expr::Formula { lhs, rhs } => {
            let lhs_vars = lhs
                .as_ref()
                .map(|e| collect_symbol_names(e))
                .unwrap_or_default();
            let rhs_vars = rhs
                .as_ref()
                .map(|e| collect_symbol_names(e))
                .unwrap_or_default();
            (lhs_vars, rhs_vars)
        }
        _ => (Vec::new(), Vec::new()),
    }
}

/// Collect all symbol names from an expression (handles +, ., and bare symbols).
fn collect_symbol_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Symbol(name) if name == "." => {
            // "." means all other columns — handled by the caller
            vec![".".to_string()]
        }
        Expr::Symbol(name) => vec![name.clone()],
        Expr::BinaryOp {
            op: BinaryOp::Add,
            lhs,
            rhs,
        } => {
            let mut names = collect_symbol_names(lhs);
            names.extend(collect_symbol_names(rhs));
            names
        }
        _ => Vec::new(),
    }
}

/// Get a column from a data frame by name.
fn df_get_column<'a>(df: &'a RList, name: &str) -> Option<&'a RValue> {
    // First try by named values
    for (col_name, val) in &df.values {
        if col_name.as_deref() == Some(name) {
            return Some(val);
        }
    }
    // Try the "names" attribute
    if let Some(names_val) = df.get_attr("names") {
        if let Some(Vector::Character(names)) = names_val.as_vector() {
            for (i, n) in names.iter().enumerate() {
                if n.as_deref() == Some(name) {
                    if let Some((_, val)) = df.values.get(i) {
                        return Some(val);
                    }
                }
            }
        }
    }
    None
}

/// Get all column names from a data frame.
fn df_column_names(df: &RList) -> Vec<String> {
    if let Some(names_val) = df.get_attr("names") {
        if let Some(Vector::Character(names)) = names_val.as_vector() {
            return names.iter().filter_map(|n| n.clone()).collect();
        }
    }
    df.values
        .iter()
        .enumerate()
        .map(|(i, (name, _))| name.clone().unwrap_or_else(|| format!("V{}", i + 1)))
        .collect()
}

/// Formula interface for aggregate: aggregate(y ~ x, data=df, FUN=mean)
fn aggregate_formula(
    formula_val: &RValue,
    positional: &[RValue],
    named: &[(String, RValue)],
    extra_named: &[(String, RValue)],
    fail_fast: bool,
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let formula_expr = match formula_val {
        RValue::Language(lang) => &*lang.inner,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "first argument must be a formula".to_string(),
            ))
        }
    };

    // Extract response and grouping variable names from the formula
    let (response_vars, grouping_vars) = extract_formula_vars(formula_expr);

    // Get the data argument (second positional or named "data")
    let data = positional
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "data").map(|(_, v)| v))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'data' is missing for formula interface".to_string(),
            )
        })?;

    let df = match data {
        RValue::List(l) => l,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                "'data' must be a data frame".to_string(),
            ))
        }
    };

    // Get FUN argument (third positional or named "FUN")
    let fun_val = positional
        .get(2)
        .or_else(|| named.iter().find(|(n, _)| n == "FUN").map(|(_, v)| v))
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "argument 'FUN' is missing".to_string(),
            )
        })?;
    let fun = match_fun(fun_val, env)?;

    let all_col_names = df_column_names(df);

    // Resolve "." in grouping vars (means all columns not in response)
    let resolved_grouping: Vec<String> = if grouping_vars.iter().any(|v| v == ".") {
        all_col_names
            .iter()
            .filter(|n| !response_vars.contains(n))
            .cloned()
            .collect()
    } else {
        grouping_vars.clone()
    };

    // Resolve "." in response vars (means all columns not in grouping)
    let resolved_response: Vec<String> = if response_vars.iter().any(|v| v == ".") {
        all_col_names
            .iter()
            .filter(|n| !resolved_grouping.contains(n))
            .cloned()
            .collect()
    } else {
        response_vars
    };

    // Extract grouping columns from the data frame
    let mut by_vectors: Vec<(Option<String>, Vec<RValue>)> = Vec::new();
    for gv_name in &resolved_grouping {
        let col = df_get_column(df, gv_name).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                format!("column '{}' not found in data frame", gv_name),
            )
        })?;
        by_vectors.push((Some(gv_name.clone()), rvalue_to_items(col)));
    }

    // For each response variable, run the standard aggregation
    let mut all_result_cols: Vec<(Option<String>, RValue)> = Vec::new();
    let mut group_cols_built = false;
    let mut n_groups = 0usize;

    for resp_name in &resolved_response {
        let resp_col = df_get_column(df, resp_name).ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                format!("column '{}' not found in data frame", resp_name),
            )
        })?;

        let x_items = rvalue_to_items(resp_col);
        let n = x_items.len();

        // Validate grouping vectors match response length
        for (i, (_, gv)) in by_vectors.iter().enumerate() {
            if gv.len() != n {
                return Err(RError::new(
                    RErrorKind::Argument,
                    format!(
                        "grouping vector {} has length {} but response '{}' has length {}",
                        i + 1,
                        gv.len(),
                        resp_name,
                        n
                    ),
                ));
            }
        }

        // Build composite group keys
        let mut group_keys: Vec<Vec<String>> = Vec::with_capacity(n);
        for i in 0..n {
            let key: Vec<String> = by_vectors
                .iter()
                .map(|(_, gv)| match &gv[i] {
                    RValue::Vector(rv) => rv
                        .inner
                        .as_character_scalar()
                        .unwrap_or_else(|| format!("{}", gv[i])),
                    other => format!("{}", other),
                })
                .collect();
            group_keys.push(key);
        }

        // Collect unique keys preserving first-seen order
        let mut unique_keys: Vec<Vec<String>> = Vec::new();
        let mut seen: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
        for key in &group_keys {
            if seen.insert(key.clone()) {
                unique_keys.push(key.clone());
            }
        }

        // Group items by composite key
        let mut groups: std::collections::HashMap<Vec<String>, Vec<RValue>> =
            std::collections::HashMap::new();
        for (item, key) in x_items.into_iter().zip(group_keys.iter()) {
            groups.entry(key.clone()).or_default().push(item);
        }

        n_groups = unique_keys.len();

        // Build grouping columns (only for the first response variable)
        if !group_cols_built {
            for (gi, _) in by_vectors.iter().enumerate() {
                let col_vals: Vec<Option<String>> = unique_keys
                    .iter()
                    .map(|key| Some(key[gi].clone()))
                    .collect();
                let col_name = by_vectors
                    .get(gi)
                    .and_then(|(n, _)| n.clone())
                    .unwrap_or_else(|| format!("Group.{}", gi + 1));
                all_result_cols.push((
                    Some(col_name),
                    RValue::vec(Vector::Character(col_vals.into())),
                ));
            }
            group_cols_built = true;
        }

        // Apply FUN to each group
        let mut result_vals: Vec<RValue> = Vec::with_capacity(n_groups);
        context.with_interpreter(|interp| {
            for key in &unique_keys {
                let items = groups.remove(key).unwrap_or_default();
                let group_vec = combine_items_to_vector(&items);
                if fail_fast {
                    let result = interp.call_function(&fun, &[group_vec], extra_named, env)?;
                    result_vals.push(result);
                } else {
                    match interp.call_function(&fun, &[group_vec], extra_named, env) {
                        Ok(result) => result_vals.push(result),
                        Err(_) => result_vals.push(RValue::Null),
                    }
                }
            }
            Ok::<(), RError>(())
        })?;

        // Add result column
        let all_scalar = result_vals.iter().all(|r| r.length() == 1);
        if all_scalar && !result_vals.is_empty() {
            let simplified = combine_items_to_vector(&result_vals);
            all_result_cols.push((Some(resp_name.clone()), simplified));
        } else {
            let entries: Vec<(Option<String>, RValue)> =
                result_vals.into_iter().map(|v| (None, v)).collect();
            all_result_cols.push((Some(resp_name.clone()), RValue::List(RList::new(entries))));
        }
    }

    let mut result = RList::new(all_result_cols);
    result.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(n_groups)?).map(Some).collect();
    result.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    let col_names: Vec<Option<String>> = result.values.iter().map(|(n, _)| n.clone()).collect();
    result.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(col_names.into())),
    );

    Ok(RValue::List(result))
}

/// Standard (non-formula) aggregate: aggregate(x, by, FUN)
fn aggregate_standard(
    x: &RValue,
    by: &RValue,
    fun: &RValue,
    extra_named: &[(String, RValue)],
    fail_fast: bool,
    env: &Environment,
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // by must be a list of grouping vectors
    let by_vectors: Vec<(Option<String>, Vec<RValue>)> = match by {
        RValue::List(l) => l
            .values
            .iter()
            .map(|(name, v)| (name.clone(), rvalue_to_items(v)))
            .collect(),
        _ => {
            // Single grouping vector — wrap in a list
            vec![(None, rvalue_to_items(by))]
        }
    };

    let x_items = rvalue_to_items(x);
    let n = x_items.len();

    // Validate all grouping vectors have the same length as x
    for (i, (_, gv)) in by_vectors.iter().enumerate() {
        if gv.len() != n {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "grouping vector {} has length {} but 'x' has length {}",
                    i + 1,
                    gv.len(),
                    n
                ),
            ));
        }
    }

    // Build composite group keys from all grouping vectors
    let mut group_keys: Vec<Vec<String>> = Vec::with_capacity(n);
    for i in 0..n {
        let key: Vec<String> = by_vectors
            .iter()
            .map(|(_, gv)| match &gv[i] {
                RValue::Vector(rv) => rv
                    .inner
                    .as_character_scalar()
                    .unwrap_or_else(|| format!("{}", gv[i])),
                other => format!("{}", other),
            })
            .collect();
        group_keys.push(key);
    }

    // Collect unique composite keys preserving first-seen order
    let mut unique_keys: Vec<Vec<String>> = Vec::new();
    let mut seen: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
    for key in &group_keys {
        if seen.insert(key.clone()) {
            unique_keys.push(key.clone());
        }
    }

    // Group x items by composite key
    let mut groups: std::collections::HashMap<Vec<String>, Vec<RValue>> =
        std::collections::HashMap::new();
    for (item, key) in x_items.into_iter().zip(group_keys.iter()) {
        groups.entry(key.clone()).or_default().push(item);
    }

    // Apply FUN to each group and build result columns
    let n_groups = unique_keys.len();
    let n_by = by_vectors.len();

    // Group columns (Group.1, Group.2, ...)
    let mut group_cols: Vec<Vec<Option<String>>> = vec![Vec::with_capacity(n_groups); n_by];
    let mut result_vals: Vec<RValue> = Vec::with_capacity(n_groups);

    context.with_interpreter(|interp| {
        for key in &unique_keys {
            for (col_idx, k) in key.iter().enumerate() {
                group_cols[col_idx].push(Some(k.clone()));
            }
            let items = groups.remove(key).unwrap_or_default();
            let group_vec = combine_items_to_vector(&items);
            if fail_fast {
                let result = interp.call_function(fun, &[group_vec], extra_named, env)?;
                result_vals.push(result);
            } else {
                match interp.call_function(fun, &[group_vec], extra_named, env) {
                    Ok(result) => result_vals.push(result),
                    Err(_) => result_vals.push(RValue::Null),
                }
            }
        }
        Ok::<(), RError>(())
    })?;

    // Build the result data frame
    let mut df_cols: Vec<(Option<String>, RValue)> = Vec::new();

    // Add grouping columns
    for (i, col) in group_cols.into_iter().enumerate() {
        let col_name = by_vectors
            .get(i)
            .and_then(|(n, _)| n.clone())
            .unwrap_or_else(|| format!("Group.{}", i + 1));
        df_cols.push((Some(col_name), RValue::vec(Vector::Character(col.into()))));
    }

    // Add the result column — try to simplify scalar results to a vector
    let all_scalar = result_vals.iter().all(|r| r.length() == 1);
    if all_scalar && !result_vals.is_empty() {
        let simplified = combine_items_to_vector(&result_vals);
        df_cols.push((Some("x".to_string()), simplified));
    } else {
        let entries: Vec<(Option<String>, RValue)> =
            result_vals.into_iter().map(|v| (None, v)).collect();
        df_cols.push((Some("x".to_string()), RValue::List(RList::new(entries))));
    }

    let mut result = RList::new(df_cols);
    result.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(n_groups)?).map(Some).collect();
    result.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    // Set names attribute
    let col_names: Vec<Option<String>> = result.values.iter().map(|(n, _)| n.clone()).collect();
    result.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(col_names.into())),
    );

    Ok(RValue::List(result))
}

// endregion

// region: outer

/// Outer product of two vectors, applying FUN to each pair of elements.
///
/// @param X first vector
/// @param Y second vector
/// @param FUN function to apply (default: "*")
/// @return matrix with dim = c(length(X), length(Y))
#[interpreter_builtin(min_args = 2)]
fn interp_outer(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();

    let x = positional
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'X' is missing".to_string()))?;
    let y = positional
        .get(1)
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'Y' is missing".to_string()))?;

    // FUN can be positional arg #3 or named
    let fun_val = named
        .iter()
        .find(|(n, _)| n == "FUN")
        .map(|(_, v)| v.clone())
        .or_else(|| positional.get(2).cloned());

    let x_items = rvalue_to_items(x);
    let y_items = rvalue_to_items(y);
    let nx = x_items.len();
    let ny = y_items.len();

    // Try to resolve as a known arithmetic operator for fast path
    let use_fast_path = match &fun_val {
        Some(RValue::Vector(rv)) => rv.inner.as_character_scalar().is_some(),
        None => true, // default is "*"
        _ => false,
    };

    if use_fast_path {
        let fun_str = fun_val
            .as_ref()
            .and_then(|v| v.as_vector()?.as_character_scalar())
            .unwrap_or_else(|| "*".to_string());

        let op: Option<fn(f64, f64) -> f64> = match fun_str.as_str() {
            "*" => Some(|a, b| a * b),
            "+" => Some(|a, b| a + b),
            "-" => Some(|a, b| a - b),
            "/" => Some(|a, b| a / b),
            "^" | "**" => Some(|a: f64, b: f64| a.powf(b)),
            "%%" => Some(|a, b| a % b),
            "%/%" => Some(|a: f64, b: f64| (a / b).floor()),
            _ => None, // Fall through to general path
        };

        if let Some(op) = op {
            let x_vec = match x {
                RValue::Vector(rv) => rv.to_doubles(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "outer() requires vectors for X and Y".to_string(),
                    ))
                }
            };
            let y_vec = match y {
                RValue::Vector(rv) => rv.to_doubles(),
                _ => {
                    return Err(RError::new(
                        RErrorKind::Argument,
                        "outer() requires vectors for X and Y".to_string(),
                    ))
                }
            };

            // R stores matrices column-major: iterate columns (Y) then rows (X)
            let mut result = Vec::with_capacity(nx * ny);
            for y_val in &y_vec {
                for x_val in &x_vec {
                    let val = match (x_val, y_val) {
                        (Some(xv), Some(yv)) => Some(op(*xv, *yv)),
                        _ => None,
                    };
                    result.push(val);
                }
            }

            return build_outer_matrix(Vector::Double(result.into()), nx, ny, x, y);
        }

        // If it's a string naming a function, look it up
        let fun_rv = match_fun(
            &RValue::vec(Vector::Character(vec![Some(fun_str)].into())),
            env,
        )?;
        return outer_general(&fun_rv, &x_items, &y_items, nx, ny, x, y, context, env);
    }

    // General path: FUN is a closure or other callable
    let fun = match_fun(fun_val.as_ref().unwrap_or(&RValue::Null), env)?;
    outer_general(&fun, &x_items, &y_items, nx, ny, x, y, context, env)
}

/// Extract the "names" attribute from an RValue as a list of optional strings.
fn outer_names(val: &RValue) -> Option<Vec<Option<String>>> {
    match val {
        RValue::Vector(rv) => rv.get_attr("names").and_then(|nv| {
            if let RValue::Vector(nrv) = nv {
                Some(nrv.inner.to_characters())
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Set dimnames on an outer product result matrix if X or Y had names.
fn set_outer_dimnames(rv: &mut RVector, x: &RValue, y: &RValue) {
    let x_names = outer_names(x);
    let y_names = outer_names(y);
    if x_names.is_some() || y_names.is_some() {
        let row_names = x_names
            .map(|n| RValue::vec(Vector::Character(n.into())))
            .unwrap_or(RValue::Null);
        let col_names = y_names
            .map(|n| RValue::vec(Vector::Character(n.into())))
            .unwrap_or(RValue::Null);
        rv.set_attr(
            "dimnames".to_string(),
            RValue::List(RList::new(vec![(None, row_names), (None, col_names)])),
        );
    }
}

/// Build an RVector matrix with class, dim, and optional dimnames.
fn build_outer_matrix(
    inner: Vector,
    nx: usize,
    ny: usize,
    x_orig: &RValue,
    y_orig: &RValue,
) -> Result<RValue, RError> {
    let mut rv = RVector::from(inner);
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("matrix".to_string()), Some("array".to_string())].into(),
        )),
    );
    rv.set_attr(
        "dim".to_string(),
        RValue::vec(Vector::Integer(
            vec![Some(i64::try_from(nx)?), Some(i64::try_from(ny)?)].into(),
        )),
    );
    set_outer_dimnames(&mut rv, x_orig, y_orig);
    Ok(RValue::Vector(rv))
}

/// General outer product: call FUN(x_i, y_j) for each pair and collect into a matrix.
#[allow(clippy::too_many_arguments)]
fn outer_general(
    fun: &RValue,
    x_items: &[RValue],
    y_items: &[RValue],
    nx: usize,
    ny: usize,
    x_orig: &RValue,
    y_orig: &RValue,
    context: &BuiltinContext,
    env: &Environment,
) -> Result<RValue, RError> {
    let mut results: Vec<RValue> = Vec::with_capacity(nx * ny);

    context.with_interpreter(|interp| {
        // Column-major order: iterate Y (columns) then X (rows)
        for yv in y_items {
            for xv in x_items {
                let result = interp.call_function(fun, &[xv.clone(), yv.clone()], &[], env)?;
                results.push(result);
            }
        }
        Ok::<(), RError>(())
    })?;

    // Try to simplify: if all results are scalar, combine into a typed vector
    let all_scalar = results.iter().all(|r| r.length() == 1);
    if all_scalar && !results.is_empty() {
        let first_type = results[0].type_name();
        let all_same = results.iter().all(|r| r.type_name() == first_type);
        if all_same {
            match first_type {
                "double" => {
                    let vals: Vec<Option<f64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return build_outer_matrix(Vector::Double(vals.into()), nx, ny, x_orig, y_orig);
                }
                "integer" => {
                    let vals: Vec<Option<i64>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_integers().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return build_outer_matrix(
                        Vector::Integer(vals.into()),
                        nx,
                        ny,
                        x_orig,
                        y_orig,
                    );
                }
                "character" => {
                    let vals: Vec<Option<String>> = results
                        .iter()
                        .filter_map(|r| {
                            r.as_vector()
                                .map(|v| v.to_characters().into_iter().next().unwrap_or(None))
                        })
                        .collect();
                    return build_outer_matrix(
                        Vector::Character(vals.into()),
                        nx,
                        ny,
                        x_orig,
                        y_orig,
                    );
                }
                _ => {}
            }
        }
    }

    // Fall back: collect all results into doubles
    let vals: Vec<Option<f64>> = results
        .iter()
        .filter_map(|r| {
            r.as_vector()
                .map(|v| v.to_doubles().into_iter().next().unwrap_or(None))
        })
        .collect();
    build_outer_matrix(Vector::Double(vals.into()), nx, ny, x_orig, y_orig)
}

// endregion

/// Summarize an object (S3 generic).
///
/// Dispatches to summary.lm, summary.data.frame, etc. when a method exists.
/// Falls back to printing the object's structure.
///
/// @param object the object to summarize
/// @return a summary of the object
#[interpreter_builtin(min_args = 1)]
fn interp_summary(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    // Try S3 dispatch (summary.lm, summary.data.frame, etc.)
    if let Some(result) = try_s3_dispatch("summary", args, named, context)? {
        return Ok(result);
    }
    // Default: for vectors, compute basic summary statistics
    match args.first() {
        Some(RValue::Vector(rv)) => {
            let doubles = rv.to_doubles();
            let vals: Vec<f64> = doubles.into_iter().flatten().collect();
            if vals.is_empty() {
                return Ok(RValue::Null);
            }
            let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
            let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let sum: f64 = vals.iter().sum();
            let mean = sum / vals.len() as f64;
            let mut sorted = vals;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = quantile_type7(&sorted, 0.5);
            let q1 = quantile_type7(&sorted, 0.25);
            let q3 = quantile_type7(&sorted, 0.75);

            let mut result_rv = RVector::from(Vector::Double(
                vec![
                    Some(min),
                    Some(q1),
                    Some(median),
                    Some(mean),
                    Some(q3),
                    Some(max),
                ]
                .into(),
            ));
            result_rv.set_attr(
                "names".to_string(),
                RValue::vec(Vector::Character(
                    vec![
                        Some("Min.".to_string()),
                        Some("1st Qu.".to_string()),
                        Some("Median".to_string()),
                        Some("Mean".to_string()),
                        Some("3rd Qu.".to_string()),
                        Some("Max.".to_string()),
                    ]
                    .into(),
                )),
            );
            Ok(RValue::Vector(result_rv))
        }
        Some(other) => Ok(other.clone()),
        None => Ok(RValue::Null),
    }
}

/// Compute a quantile using R's type 7 algorithm (the default).
///
/// For sorted data of length n and probability p:
/// h = (n - 1) * p, result = x[floor(h)] + (h - floor(h)) * (x[ceil(h)] - x[floor(h)])
fn quantile_type7(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return sorted[0];
    }
    let h = (n - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    let frac = h - h.floor();
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

// region: reg.finalizer

/// Register a function to be called when an environment is garbage collected,
/// or at interpreter exit if `onexit = TRUE`.
///
/// Since miniR uses Rc-based environments (no tracing GC), finalizers with
/// `onexit = FALSE` are accepted silently but will never fire. When
/// `onexit = TRUE`, the finalizer is stored on the Interpreter and executed
/// during its Drop.
///
/// @param e an environment to attach the finalizer to
/// @param f a function of one argument (the environment) to call
/// @param onexit logical; if TRUE, run the finalizer at interpreter exit
/// @return NULL, invisibly
#[interpreter_builtin(name = "reg.finalizer", min_args = 2, max_args = 3)]
fn interp_reg_finalizer(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    // e — must be an environment
    let e = call_args.value("e", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires an environment as its first argument".to_string(),
        )
    })?;
    if !matches!(e, RValue::Environment(_)) {
        return Err(RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires an environment as its first argument".to_string(),
        ));
    }

    // f — must be a function
    let f = call_args.value("f", 1).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "reg.finalizer() requires a function as its second argument".to_string(),
        )
    })?;
    let f = match_fun(f, context.env())?;

    // onexit — logical, default FALSE
    let onexit = call_args.logical_flag("onexit", 2, false);

    if onexit {
        context.with_interpreter(|interp| {
            interp.finalizers.borrow_mut().push(f);
        });
    }
    // When onexit is FALSE, we accept silently — no GC means it won't fire,
    // but it shouldn't error either.

    Ok(RValue::Null)
}

// endregion

// region: options

/// Get or set global options.
///
/// With no arguments, returns all current options as a named list.
/// With character arguments, returns the named options.
/// With name=value pairs, sets those options and returns the previous values.
///
/// @param ... option names to query, or name=value pairs to set
/// @return list of (previous) option values
#[interpreter_builtin]
fn interp_options(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let mut result: Vec<(Option<String>, RValue)> = Vec::new();

        // If no arguments, return all options
        if positional.is_empty() && named.is_empty() {
            let opts = interp.options.borrow();
            let mut entries: Vec<_> = opts.iter().collect();
            entries.sort_by_key(|(k, _)| (*k).clone());
            for (k, v) in entries {
                result.push((Some(k.clone()), v.clone()));
            }
            return Ok(RValue::List(RList::new(result)));
        }

        // Process positional args — character strings are queries
        for arg in positional {
            if let Some(name) = arg.as_vector().and_then(|v| v.as_character_scalar()) {
                let val = interp
                    .options
                    .borrow()
                    .get(&name)
                    .cloned()
                    .unwrap_or(RValue::Null);
                result.push((Some(name), val));
            } else if let RValue::List(list) = arg {
                // Setting options from a list (e.g. options(old_opts))
                for (opt_name, val) in &list.values {
                    if let Some(opt_name) = opt_name {
                        let prev = interp
                            .options
                            .borrow()
                            .get(opt_name.as_str())
                            .cloned()
                            .unwrap_or(RValue::Null);
                        interp
                            .options
                            .borrow_mut()
                            .insert(opt_name.clone(), val.clone());
                        result.push((Some(opt_name.clone()), prev));
                    }
                }
            }
        }

        // Process named args — these are set operations
        for (name, val) in named {
            let prev = interp
                .options
                .borrow()
                .get(name)
                .cloned()
                .unwrap_or(RValue::Null);
            interp
                .options
                .borrow_mut()
                .insert(name.clone(), val.clone());
            result.push((Some(name.clone()), prev));
        }

        Ok(RValue::List(RList::new(result)))
    })
}

/// Get the value of a named global option.
///
/// @param name character string — the option name
/// @param default value to return if the option is not set (default NULL)
/// @return the option value, or default if not set
#[interpreter_builtin(name = "getOption", min_args = 1)]
fn interp_get_option(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = positional
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "getOption() requires a character string as its first argument".to_string(),
            )
        })?;
    let default = positional.get(1).cloned().unwrap_or(RValue::Null);

    context.with_interpreter(|interp| {
        Ok(interp
            .options
            .borrow()
            .get(&name)
            .cloned()
            .unwrap_or(default))
    })
}

// endregion

// region: match.call, Find, Position, Negate, rapply

/// Return the call expression with arguments matched to formal parameters.
///
/// Reconstructs the call as if all arguments were named according to the
/// function's formal parameter list. Useful for programming on the language.
///
/// @param definition the function whose formals to match against (default: parent function)
/// @param call the call to match (default: parent's call)
/// @return language object with matched arguments
#[interpreter_builtin(name = "match.call")]
fn interp_match_call(
    _positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    context.with_interpreter(|interp| {
        let frame = interp
            .current_call_frame()
            .ok_or_else(|| RError::other("match.call() must be called from within a function"))?;

        // Get the formals from the function
        let params: Vec<Param> = match &frame.function {
            RValue::Function(RFunction::Closure { params, .. }) => params.clone(),
            _ => Vec::new(),
        };

        // Get the original call expression
        let call_expr = frame
            .call
            .ok_or_else(|| RError::other("match.call() requires a call expression on the stack"))?;

        // Extract the function name from the call
        let func_expr = match &call_expr {
            Expr::Call { func, .. } => (**func).clone(),
            _ => return Ok(RValue::Language(Language::new(call_expr))),
        };

        // Reconstruct with matched argument names
        let positional = &frame.supplied_positional;
        let named = &frame.supplied_named;

        // Simplified 3-pass matching to figure out which positional maps to which formal
        let formal_names: Vec<&str> = params
            .iter()
            .filter(|p| !p.is_dots)
            .map(|p| p.name.as_str())
            .collect();

        let mut named_to_formal: std::collections::HashMap<usize, &str> =
            std::collections::HashMap::new();
        let mut matched_formals: std::collections::HashSet<&str> = std::collections::HashSet::new();

        // Pass 1: exact name match
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if let Some(&formal) = formal_names.iter().find(|&&f| f == arg_name) {
                if !matched_formals.contains(formal) {
                    matched_formals.insert(formal);
                    named_to_formal.insert(i, formal);
                }
            }
        }

        // Pass 2: partial match
        for (i, (arg_name, _)) in named.iter().enumerate() {
            if named_to_formal.contains_key(&i) {
                continue;
            }
            let candidates: Vec<&str> = formal_names
                .iter()
                .filter(|&&f| !matched_formals.contains(f) && f.starts_with(arg_name.as_str()))
                .copied()
                .collect();
            if candidates.len() == 1 {
                matched_formals.insert(candidates[0]);
                named_to_formal.insert(i, candidates[0]);
            }
        }

        // Build reverse map
        let formal_to_named: std::collections::HashMap<&str, usize> = named_to_formal
            .iter()
            .map(|(&idx, &formal)| (formal, idx))
            .collect();

        // Reconstruct args in formal order
        let mut result_args: Vec<Arg> = Vec::new();
        let mut pos_idx = 0usize;

        for param in &params {
            if param.is_dots {
                // Collect remaining positional
                while pos_idx < positional.len() {
                    result_args.push(Arg {
                        name: None,
                        value: Some(rvalue_to_expr(&positional[pos_idx])),
                    });
                    pos_idx += 1;
                }
                // Collect unmatched named
                for (i, (name, val)) in named.iter().enumerate() {
                    if !named_to_formal.contains_key(&i) {
                        result_args.push(Arg {
                            name: Some(name.clone()),
                            value: Some(rvalue_to_expr(val)),
                        });
                    }
                }
                continue;
            }

            if let Some(&named_idx) = formal_to_named.get(param.name.as_str()) {
                result_args.push(Arg {
                    name: Some(param.name.clone()),
                    value: Some(rvalue_to_expr(&named[named_idx].1)),
                });
            } else if pos_idx < positional.len() {
                result_args.push(Arg {
                    name: Some(param.name.clone()),
                    value: Some(rvalue_to_expr(&positional[pos_idx])),
                });
                pos_idx += 1;
            }
            // Skip unmatched formals with defaults
        }

        let matched_call = Expr::Call {
            func: Box::new(func_expr),
            args: result_args,
            span: None,
        };
        Ok(RValue::Language(Language::new(matched_call)))
    })
}

/// Convert an RValue to an Expr for use in match.call() reconstructed calls.
fn rvalue_to_expr(val: &RValue) -> Expr {
    match val {
        RValue::Null => Expr::Null,
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(d) if d.len() == 1 => match d.get_opt(0) {
                Some(v) if v.is_infinite() && v > 0.0 => Expr::Inf,
                Some(v) if v.is_nan() => Expr::NaN,
                Some(v) => Expr::Double(v),
                None => Expr::Na(crate::parser::ast::NaType::Real),
            },
            Vector::Integer(i) if i.len() == 1 => match i.get_opt(0) {
                Some(v) => Expr::Integer(v),
                None => Expr::Na(crate::parser::ast::NaType::Integer),
            },
            Vector::Logical(l) if l.len() == 1 => match l[0] {
                Some(v) => Expr::Bool(v),
                None => Expr::Na(crate::parser::ast::NaType::Logical),
            },
            Vector::Character(c) if c.len() == 1 => match &c[0] {
                Some(v) => Expr::String(v.clone()),
                None => Expr::Na(crate::parser::ast::NaType::Character),
            },
            _ => Expr::Symbol(format!("{}", val)),
        },
        RValue::Language(lang) => (*lang.inner).clone(),
        _ => Expr::Symbol(format!("{}", val)),
    }
}

/// Find the first element of a vector for which a predicate returns TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to search
/// @param right if TRUE, search from right to left
/// @return the first matching element, or NULL if none found
#[interpreter_builtin(name = "Find", min_args = 2)]
fn interp_find(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Find requires 2 arguments: f and x".to_string(),
        ));
    }
    let env = context.env();
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let right = named
        .iter()
        .find(|(n, _)| n == "right")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    context.with_interpreter(|interp| {
        let iter: Box<dyn Iterator<Item = &RValue>> = if right {
            Box::new(items.iter().rev())
        } else {
            Box::new(items.iter())
        };

        for item in iter {
            let result = interp.call_function(&f, std::slice::from_ref(item), &[], env)?;
            if result
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false)
            {
                return Ok(item.clone());
            }
        }
        Ok(RValue::Null)
    })
}

/// Find the position (1-based index) of the first element where a predicate is TRUE.
///
/// @param f predicate function returning a logical scalar
/// @param x vector or list to search
/// @param right if TRUE, search from right to left
/// @return scalar integer position, or NULL if none found
#[interpreter_builtin(name = "Position", min_args = 2)]
fn interp_position(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "Position requires 2 arguments: f and x".to_string(),
        ));
    }
    let env = context.env();
    let f = match_fun(&positional[0], env)?;
    let x = &positional[1];

    let right = named
        .iter()
        .find(|(n, _)| n == "right")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(false);

    let items: Vec<RValue> = rvalue_to_items(x);

    context.with_interpreter(|interp| {
        let indices: Box<dyn Iterator<Item = usize>> = if right {
            Box::new((0..items.len()).rev())
        } else {
            Box::new(0..items.len())
        };

        for i in indices {
            let result = interp.call_function(&f, std::slice::from_ref(&items[i]), &[], env)?;
            if result
                .as_vector()
                .and_then(|v| v.as_logical_scalar())
                .unwrap_or(false)
            {
                let pos = i64::try_from(i + 1).map_err(RError::from)?;
                return Ok(RValue::vec(Vector::Integer(vec![Some(pos)].into())));
            }
        }
        Ok(RValue::Null)
    })
}

/// Negate a predicate function, returning a new function that returns the
/// logical complement of the original.
///
/// @param f predicate function
/// @return a new closure that calls f and negates the result
#[interpreter_builtin(name = "Negate", min_args = 1)]
fn interp_negate(
    positional: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let env = context.env();
    let f = match_fun(&positional[0], env)?;

    // Create an environment that captures the original function
    let closure_env = Environment::new_child(env);
    closure_env.set(".negate_f".to_string(), f);

    // Build: function(...) !.negate_f(...)
    let body = Expr::UnaryOp {
        op: UnaryOp::Not,
        operand: Box::new(Expr::Call {
            func: Box::new(Expr::Symbol(".negate_f".to_string())),
            span: None,
            args: vec![Arg {
                name: None,
                value: Some(Expr::Dots),
            }],
        }),
    };

    Ok(RValue::Function(RFunction::Closure {
        params: vec![Param {
            name: "...".to_string(),
            default: None,
            is_dots: true,
        }],
        body,
        env: closure_env,
    }))
}

/// Recursively apply a function to elements of a (nested) list.
///
/// When `classes` is specified, only leaf elements whose type matches one of the
/// given class names are transformed by `f`. Non-matching leaves are replaced
/// by `deflt` (or left unchanged in "replace" mode).
///
/// @param object a list (possibly nested)
/// @param f function to apply to matching leaf elements
/// @param classes character vector of class names to match (default: "ANY" matches all)
/// @param deflt default value for non-matching leaves (used with "unlist"/"list" modes)
/// @param how one of "unlist" (default), "replace", or "list"
/// @return depends on `how`: "unlist" returns a flat vector, "replace" returns a list
///   with the same structure, "list" returns a flat list of results
#[interpreter_builtin(name = "rapply", min_args = 2)]
fn interp_rapply(
    positional: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    if positional.len() < 2 {
        return Err(RError::new(
            RErrorKind::Argument,
            "rapply requires at least 2 arguments: object and f".to_string(),
        ));
    }
    let env = context.env();
    let object = &positional[0];
    let f = match_fun(&positional[1], env)?;

    let how = named
        .iter()
        .find(|(n, _)| n == "how")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .or_else(|| {
            positional
                .get(2)
                .and_then(|v| v.as_vector()?.as_character_scalar())
        })
        .unwrap_or_else(|| "unlist".to_string());

    // Extract classes parameter: a character vector of type names to match
    let classes: Option<Vec<String>> =
        named
            .iter()
            .find(|(n, _)| n == "classes")
            .and_then(|(_, v)| match v.as_vector() {
                Some(rv) => {
                    let chars = rv.to_characters();
                    let strs: Vec<String> = chars.into_iter().flatten().collect();
                    if strs.len() == 1 && strs[0] == "ANY" {
                        None // "ANY" means match everything (same as no filter)
                    } else {
                        Some(strs)
                    }
                }
                None => None,
            });

    // Extract deflt parameter: default value for non-matching leaves
    let deflt = named
        .iter()
        .find(|(n, _)| n == "deflt")
        .map(|(_, v)| v.clone());

    context.with_interpreter(|interp| match how.as_str() {
        "replace" => rapply_replace(interp, object, &f, env, classes.as_deref()),
        "list" => {
            let mut results = Vec::new();
            rapply_collect(
                interp,
                object,
                &f,
                env,
                &mut results,
                classes.as_deref(),
                deflt.as_ref(),
            )?;
            Ok(RValue::List(RList::new(
                results.into_iter().map(|v| (None, v)).collect(),
            )))
        }
        _ => {
            // "unlist" (default)
            let mut results = Vec::new();
            rapply_collect(
                interp,
                object,
                &f,
                env,
                &mut results,
                classes.as_deref(),
                deflt.as_ref(),
            )?;
            if results.is_empty() {
                return Ok(RValue::Null);
            }
            // Try to simplify to a vector via c()
            crate::interpreter::builtins::builtin_c(&results, &[])
        }
    })
}

/// Check if an RValue's type name matches one of the given class names.
fn rapply_matches_class(x: &RValue, classes: Option<&[String]>) -> bool {
    match classes {
        None => true, // No filter — match everything
        Some(cls) => {
            let type_name = x.type_name();
            // Map R type names: "double" -> "numeric", etc.
            cls.iter().any(|c| {
                c == type_name
                    || (c == "numeric" && (type_name == "double" || type_name == "integer"))
                    || (c == "character" && type_name == "character")
                    || (c == "logical" && type_name == "logical")
                    || (c == "complex" && type_name == "complex")
                    || (c == "integer" && type_name == "integer")
                    || (c == "double" && type_name == "double")
            })
        }
    }
}

/// Helper: collect results of applying f to matching leaf (non-list) elements.
fn rapply_collect(
    interp: &crate::interpreter::Interpreter,
    x: &RValue,
    f: &RValue,
    env: &Environment,
    out: &mut Vec<RValue>,
    classes: Option<&[String]>,
    deflt: Option<&RValue>,
) -> Result<(), RError> {
    match x {
        RValue::List(list) => {
            for (_, val) in &list.values {
                rapply_collect(interp, val, f, env, out, classes, deflt)?;
            }
        }
        _ => {
            if rapply_matches_class(x, classes) {
                let result = interp
                    .call_function(f, std::slice::from_ref(x), &[], env)
                    .map_err(RError::from)?;
                out.push(result);
            } else if let Some(d) = deflt {
                out.push(d.clone());
            }
            // If no deflt and class doesn't match, skip the element
        }
    }
    Ok(())
}

/// Helper: recursively apply f, preserving list structure ("replace" mode).
fn rapply_replace(
    interp: &crate::interpreter::Interpreter,
    x: &RValue,
    f: &RValue,
    env: &Environment,
    classes: Option<&[String]>,
) -> Result<RValue, RError> {
    match x {
        RValue::List(list) => {
            let new_vals: Vec<(Option<String>, RValue)> = list
                .values
                .iter()
                .map(|(name, val)| {
                    let new_val = rapply_replace(interp, val, f, env, classes)?;
                    Ok((name.clone(), new_val))
                })
                .collect::<Result<Vec<_>, RError>>()?;
            Ok(RValue::List(RList::new(new_vals)))
        }
        _ => {
            if rapply_matches_class(x, classes) {
                Ok(interp
                    .call_function(f, std::slice::from_ref(x), &[], env)
                    .map_err(RError::from)?)
            } else {
                // Non-matching leaf: keep as-is in "replace" mode
                Ok(x.clone())
            }
        }
    }
}

// endregion

// region: Introspection — search path, namespace exploration, function lookup

/// Return the search path as a character vector.
///
/// The search path represents the order in which environments are searched
/// for names: .GlobalEnv -> attached packages -> package:base.
///
/// @return character vector of environment names on the search path
#[interpreter_builtin(name = "search")]
fn interp_search(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let path = context.with_interpreter(|interp| interp.get_search_path());
    Ok(RValue::vec(Vector::Character(
        path.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

/// List all loaded namespace names.
///
/// Returns names of all namespaces that have been loaded (via library(),
/// loadNamespace(), etc.), plus builtin namespaces.
///
/// @return character vector of namespace names
#[interpreter_builtin(name = "loadedNamespaces")]
fn interp_loaded_namespaces(
    _args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let mut namespaces: Vec<String> = super::BUILTIN_REGISTRY
        .iter()
        .map(|d| d.namespace.to_string())
        .filter(|ns| !ns.is_empty())
        .collect();

    // Add loaded package namespaces
    context.with_interpreter(|interp| {
        for name in interp.loaded_namespaces.borrow().keys() {
            namespaces.push(name.clone());
        }
    });

    namespaces.sort();
    namespaces.dedup();
    Ok(RValue::vec(Vector::Character(
        namespaces.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

/// Get exports from a namespace (list functions in a package).
///
/// @param ns character scalar: namespace name (e.g. "base", "stats", "utils")
/// @return character vector of function names in that namespace
/// @namespace base
#[interpreter_builtin(name = "getNamespaceExports", min_args = 1)]
fn interp_get_namespace_exports(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ns = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid namespace name".to_string()))?;

    // Check loaded packages first
    let loaded_exports = context.with_interpreter(|interp| {
        interp
            .loaded_namespaces
            .borrow()
            .get(&ns)
            .map(|loaded| loaded.exports_env.ls())
    });

    if let Some(mut names) = loaded_exports {
        names.sort();
        return Ok(RValue::vec(Vector::Character(
            names.into_iter().map(Some).collect::<Vec<_>>().into(),
        )));
    }

    // Fall back to builtin registry
    let mut names: Vec<String> = super::BUILTIN_REGISTRY
        .iter()
        .filter(|d| d.namespace == ns)
        .map(|d| d.name.to_string())
        .collect();
    names.sort();
    Ok(RValue::vec(Vector::Character(
        names.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

/// Find which namespace a function belongs to.
///
/// @param what character scalar: function name to look up
/// @return character vector of namespace names where the function is registered
/// @namespace utils
#[interpreter_builtin(name = "find", min_args = 1, namespace = "utils")]
fn interp_find_on_search_path(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let name = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid 'what' argument".to_string()))?;

    let mut found = Vec::new();

    // Check global env
    if context.env().get(&name).is_some() {
        found.push(".GlobalEnv".to_string());
    }

    // Check loaded packages on search path
    context.with_interpreter(|interp| {
        for entry in interp.search_path.borrow().iter() {
            if entry.env.has_local(&name) {
                found.push(entry.name.clone());
            }
        }
    });

    // Check builtin registry
    for d in super::BUILTIN_REGISTRY.iter() {
        if d.name == name {
            found.push("package:base".to_string());
            break;
        }
    }

    found.dedup();
    Ok(RValue::vec(Vector::Character(
        found.into_iter().map(Some).collect::<Vec<_>>().into(),
    )))
}

/// Get a namespace environment by name.
///
/// Returns the namespace environment for a loaded package. If the namespace
/// is not yet loaded, attempts to load it (like GNU R's getNamespace).
/// Falls back to the base environment for builtin namespaces like "base".
///
/// @param ns character scalar: namespace name
/// @return environment
/// @namespace base
#[interpreter_builtin(name = "getNamespace", min_args = 1)]
fn interp_get_namespace(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ns = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid namespace name".to_string()))?;

    // Check loaded packages first
    let loaded_ns = context.with_interpreter(|interp| {
        interp
            .loaded_namespaces
            .borrow()
            .get(&ns)
            .map(|loaded| loaded.namespace_env.clone())
    });

    if let Some(env) = loaded_ns {
        return Ok(RValue::Environment(env));
    }

    // Try to load the namespace if it's not already loaded
    let loaded_env = context.with_interpreter(|interp| interp.load_namespace(&ns).ok());

    if let Some(env) = loaded_env {
        return Ok(RValue::Environment(env));
    }

    // Fall back to base env for builtin namespaces (base, utils, stats, etc.)
    let env = context.with_interpreter(|interp| interp.base_env());
    Ok(RValue::Environment(env))
}

/// Check if a namespace is loaded.
///
/// @param ns character scalar: namespace name
/// @return logical scalar
/// @namespace base
#[interpreter_builtin(name = "isNamespaceLoaded", min_args = 1)]
fn interp_is_namespace_loaded(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ns = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::new(RErrorKind::Argument, "invalid namespace name".to_string()))?;

    // Check loaded packages
    let loaded =
        context.with_interpreter(|interp| interp.loaded_namespaces.borrow().contains_key(&ns));

    if loaded {
        return Ok(RValue::vec(Vector::Logical(vec![Some(true)].into())));
    }

    // Fall back to builtin registry
    let exists = super::BUILTIN_REGISTRY.iter().any(|d| d.namespace == ns);
    Ok(RValue::vec(Vector::Logical(vec![Some(exists)].into())))
}

/// Get the version of an installed package from its DESCRIPTION file.
///
/// @param pkg character scalar: the package name
/// @param lib.loc character vector: library paths to search (defaults to .libPaths())
/// @return character scalar: the version string (e.g. "1.4.2")
/// @namespace utils
#[interpreter_builtin(name = "packageVersion", min_args = 1, namespace = "utils")]
fn interp_package_version(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'pkg' must be a character string".to_string(),
            )
        })?;

    // Optional lib.loc argument
    let lib_loc: Option<Vec<String>> =
        named
            .iter()
            .find(|(n, _)| n == "lib.loc")
            .and_then(|(_, v)| {
                let vec = v.as_vector()?;
                Some(
                    vec.to_characters()
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>(),
                )
            });

    let version = context.with_interpreter(|interp| {
        // Check loaded namespaces first — avoids re-reading DESCRIPTION from disk
        if let Some(ns) = interp.loaded_namespaces.borrow().get(&pkg) {
            return Some(ns.description.version.clone());
        }

        // Search on disk
        let lib_paths = lib_loc.unwrap_or_else(|| interp.get_lib_paths());
        for lib_path in &lib_paths {
            let desc_path = std::path::Path::new(lib_path)
                .join(&pkg)
                .join("DESCRIPTION");
            if let Ok(text) = std::fs::read_to_string(&desc_path) {
                if let Ok(desc) = crate::interpreter::packages::PackageDescription::parse(&text) {
                    return Some(desc.version);
                }
            }
        }
        None
    });

    match version {
        Some(v) => Ok(RValue::vec(Vector::Character(vec![Some(v)].into()))),
        None => Err(RError::new(
            RErrorKind::Other,
            format!(
                "package '{}' not found\n  \
                 Hint: check that the package is installed in one of the library paths \
                 returned by .libPaths()",
                pkg
            ),
        )),
    }
}

/// Read the DESCRIPTION file for a package, returning all fields as a named list.
///
/// @param pkg character scalar: package name
/// @param lib.loc character vector of library paths (default: .libPaths())
/// @param fields character vector of fields to return (default: all)
/// @return a named list (with class "packageDescription") of DESCRIPTION fields
/// @namespace utils
#[interpreter_builtin(name = "packageDescription", min_args = 1, namespace = "utils")]
fn interp_package_description(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let pkg = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| {
            RError::new(
                RErrorKind::Argument,
                "'pkg' must be a character string".to_string(),
            )
        })?;

    let lib_loc: Option<Vec<String>> =
        named
            .iter()
            .find(|(n, _)| n == "lib.loc")
            .and_then(|(_, v)| {
                let vec = v.as_vector()?;
                Some(
                    vec.to_characters()
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>(),
                )
            });

    let fields_filter: Option<Vec<String>> =
        named
            .iter()
            .find(|(n, _)| n == "fields")
            .and_then(|(_, v)| {
                let vec = v.as_vector()?;
                Some(
                    vec.to_characters()
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>(),
                )
            });

    let desc_fields = context.with_interpreter(|interp| {
        // Synthetic DESCRIPTION for base packages
        if crate::interpreter::Interpreter::is_base_package(&pkg) {
            let mut fields = std::collections::HashMap::new();
            fields.insert("Package".to_string(), pkg.clone());
            fields.insert("Version".to_string(), "4.4.0".to_string());
            fields.insert("Priority".to_string(), "base".to_string());
            fields.insert("Title".to_string(), format!("The R {pkg} Package"));
            return Some(fields);
        }

        // Check loaded namespaces first
        if let Some(ns) = interp.loaded_namespaces.borrow().get(&pkg) {
            return Some(ns.description.fields.clone());
        }

        // Search on disk
        let lib_paths = lib_loc.unwrap_or_else(|| interp.get_lib_paths());
        for lib_path in &lib_paths {
            let desc_path = std::path::Path::new(lib_path)
                .join(&pkg)
                .join("DESCRIPTION");
            if let Ok(text) = std::fs::read_to_string(&desc_path) {
                if let Ok(desc) = crate::interpreter::packages::PackageDescription::parse(&text) {
                    return Some(desc.fields);
                }
            }
        }
        None
    });

    match desc_fields {
        Some(fields) => {
            let mut values: Vec<(Option<String>, RValue)> = Vec::new();
            for (key, val) in &fields {
                if let Some(ref filter) = fields_filter {
                    if !filter.iter().any(|f| f == key) {
                        continue;
                    }
                }
                values.push((
                    Some(key.clone()),
                    RValue::vec(Vector::Character(vec![Some(val.clone())].into())),
                ));
            }
            let mut list = RList::new(values);
            let mut attrs = indexmap::IndexMap::new();
            attrs.insert(
                "class".to_string(),
                RValue::vec(Vector::Character(
                    vec![Some("packageDescription".to_string())].into(),
                )),
            );
            list.attrs = Some(Box::new(attrs));
            Ok(RValue::List(list))
        }
        None => Err(RError::new(
            RErrorKind::Other,
            format!("package '{}' not found", pkg),
        )),
    }
}

/// Return the number of builtins registered, optionally filtered by namespace.
///
/// This is a miniR extension — not in GNU R. Useful for debugging.
///
/// @param ns optional character scalar: namespace to filter by
/// @return integer scalar
/// @namespace base
#[interpreter_builtin(name = ".builtinCount")]
fn interp_builtin_count(
    args: &[RValue],
    _named: &[(String, RValue)],
    _context: &BuiltinContext,
) -> Result<RValue, RError> {
    let ns_filter = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar());

    let count = match ns_filter {
        Some(ns) => super::BUILTIN_REGISTRY
            .iter()
            .filter(|d| d.namespace == ns)
            .count(),
        None => super::BUILTIN_REGISTRY.len(),
    };

    Ok(RValue::vec(Vector::Integer(
        vec![Some(i64::try_from(count).unwrap_or(0))].into(),
    )))
}

// endregion
