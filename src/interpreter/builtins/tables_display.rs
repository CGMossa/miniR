//! Rich table display builtins using the `tabled` crate.
//!
//! Provides `View()` for interactive data.frame viewing and `kable()` for
//! markdown-formatted table output. Also exports helpers used by `str()`.

use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::{builtin, interpreter_builtin};
use tabled::settings::style::Style;
use tabled::settings::{Modify, Width};
use tabled::{builder::Builder, settings::object::Columns};

use super::has_class;

// region: data frame extraction helpers

/// Extracted table data from an R data.frame (or matrix).
struct TableData {
    col_names: Vec<String>,
    col_types: Vec<&'static str>,
    row_names: Vec<String>,
    /// Each inner Vec is one column's formatted values.
    columns: Vec<Vec<String>>,
    nrow: usize,
}

/// Extract table data from an RValue that is a data.frame.
///
/// Returns `None` if the value is not a data.frame list.
fn extract_data_frame(val: &RValue) -> Option<TableData> {
    let list = match val {
        RValue::List(l) if has_class(val, "data.frame") => l,
        _ => return None,
    };

    if list.values.is_empty() {
        return Some(TableData {
            col_names: Vec::new(),
            col_types: Vec::new(),
            row_names: Vec::new(),
            columns: Vec::new(),
            nrow: 0,
        });
    }

    let col_names: Vec<String> = list
        .values
        .iter()
        .enumerate()
        .map(|(i, (name, _))| name.clone().unwrap_or_else(|| format!("V{}", i + 1)))
        .collect();

    let nrow = list
        .get_attr("row.names")
        .map(|v| v.length())
        .unwrap_or_else(|| list.values.first().map(|(_, v)| v.length()).unwrap_or(0));

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

    let col_types: Vec<&'static str> = list
        .values
        .iter()
        .map(|(_, value)| match value {
            RValue::Vector(rv) => rv.inner.type_name(),
            RValue::Null => "NULL",
            _ => "list",
        })
        .collect();

    let columns: Vec<Vec<String>> = list
        .values
        .iter()
        .map(|(_, value)| match value {
            RValue::Vector(rv) => format_column_values(&rv.inner, nrow),
            RValue::Null => vec!["NULL".to_string(); nrow],
            other => vec![format!("{}", other); nrow],
        })
        .collect();

    Some(TableData {
        col_names,
        col_types,
        row_names,
        columns,
        nrow,
    })
}

/// Format individual elements of a vector column.
fn format_column_values(v: &Vector, nrow: usize) -> Vec<String> {
    use crate::interpreter::value::vector::{format_r_complex, format_r_double};

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
                Vector::Integer(vals) => match vals[i] {
                    Some(n) => n.to_string(),
                    None => "NA".to_string(),
                },
                Vector::Double(vals) => match vals[i] {
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

// endregion

// region: View()

/// Display a data.frame as a nicely formatted table.
///
/// Terminal equivalent of RStudio's View pane. Shows column headers with types,
/// row numbers, and truncates wide columns (max 30 chars) and long data frames
/// (first 20 rows + "... N more rows" footer).
///
/// @param x a data.frame to display
/// @param title optional title (unused in terminal mode)
/// @return x, invisibly
#[interpreter_builtin(name = "View", min_args = 1)]
fn interp_view(
    args: &[RValue],
    _named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let val = &args[0];

    let data = extract_data_frame(val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "View() requires a data.frame. Use as.data.frame() to convert other objects."
                .to_string(),
        )
    })?;

    // If a GUI channel is available, send the data as a View tab
    #[cfg(feature = "plot")]
    {
        let tx = context.interpreter().plot_tx.borrow();
        if let Some(tx) = tx.as_ref() {
            use crate::interpreter::graphics::view::ColType;
            let table_data = crate::interpreter::graphics::view::TableData {
                title: "View".to_string(),
                headers: data.col_names.clone(),
                col_types: data
                    .col_types
                    .iter()
                    .map(|t| match *t {
                        "dbl" => ColType::Double,
                        "int" => ColType::Integer,
                        "chr" => ColType::Character,
                        "lgl" => ColType::Logical,
                        _ => ColType::Other,
                    })
                    .collect(),
                row_names: data.row_names.clone(),
                rows: {
                    let nrow = data.nrow;
                    let ncol = data.columns.len();
                    (0..nrow)
                        .map(|r| {
                            (0..ncol)
                                .map(|c| {
                                    data.columns
                                        .get(c)
                                        .and_then(|col| col.get(r).cloned())
                                        .unwrap_or_else(|| "NA".to_string())
                                })
                                .collect()
                        })
                        .collect()
                },
            };
            let _ =
                tx.send(crate::interpreter::graphics::egui_device::PlotMessage::View(table_data));
            context.interpreter().set_invisible();
            return Ok(val.clone());
        }
    }

    if data.nrow == 0 {
        context.write(&format!(
            "data frame with 0 rows and {} columns: {}\n",
            data.col_names.len(),
            data.col_names.join(", ")
        ));
        return Ok(val.clone());
    }

    let max_display_rows: usize = 20;
    let max_col_width: usize = 30;
    let display_rows = data.nrow.min(max_display_rows);

    // Build header: column name <type>
    let headers: Vec<String> = std::iter::once(String::new()) // row-name column
        .chain(
            data.col_names
                .iter()
                .zip(data.col_types.iter())
                .map(|(name, ty)| format!("{} <{}>", name, short_type_name(ty))),
        )
        .collect();

    let mut builder = Builder::new();
    builder.push_record(&headers);

    for row in 0..display_rows {
        let row_name = data
            .row_names
            .get(row)
            .cloned()
            .unwrap_or_else(|| (row + 1).to_string());
        let mut cells: Vec<String> = vec![row_name];
        for col in &data.columns {
            cells.push(col.get(row).cloned().unwrap_or_else(|| "NA".to_string()));
        }
        builder.push_record(&cells);
    }

    let mut table = builder.build();
    table
        .with(Style::rounded())
        .with(Modify::new(Columns::new(1..)).with(Width::truncate(max_col_width).suffix("...")));

    context.write(&format!("{}\n", table));

    if data.nrow > max_display_rows {
        context.write(&format!(
            "... {} more rows ({} total)\n",
            data.nrow - max_display_rows,
            data.nrow
        ));
    }

    Ok(val.clone())
}

// endregion

// region: kable()

/// Render a data.frame as a markdown/pipe table.
///
/// Simplified version of `knitr::kable(x, format = "pipe")`. Produces a
/// markdown-formatted table suitable for inclusion in reports.
///
/// @param x a data.frame
/// @param format table format: "pipe" (default) or "simple"
/// @return character string containing the formatted table
#[builtin(min_args = 1)]
fn builtin_kable(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let val = &args[0];

    let data = extract_data_frame(val).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "kable() requires a data.frame. Use as.data.frame() to convert other objects."
                .to_string(),
        )
    })?;

    // Parse format argument (default: "pipe" = markdown)
    let format_owned = named
        .iter()
        .find(|(n, _)| n == "format")
        .and_then(|(_, v)| v.as_vector())
        .and_then(|v| v.as_character_scalar());
    let format = format_owned.as_deref().unwrap_or("pipe");

    if data.nrow == 0 {
        let header = data.col_names.join(" | ");
        return Ok(RValue::vec(Vector::Character(vec![Some(header)].into())));
    }

    // Build the table
    let headers: Vec<String> = data.col_names.clone();

    let mut builder = Builder::new();
    builder.push_record(&headers);

    for row in 0..data.nrow {
        let cells: Vec<String> = data
            .columns
            .iter()
            .map(|col| col.get(row).cloned().unwrap_or_else(|| "NA".to_string()))
            .collect();
        builder.push_record(&cells);
    }

    let mut table = builder.build();

    match format {
        "pipe" | "markdown" => {
            table.with(Style::markdown());
        }
        "simple" => {
            table.with(Style::psql());
        }
        _ => {
            table.with(Style::markdown());
        }
    }

    let output = table.to_string();
    Ok(RValue::vec(Vector::Character(vec![Some(output)].into())))
}

// endregion

// region: str() data.frame helper

/// Format `str()` output for a data.frame using tabled for alignment.
///
/// Produces output like R's `str()`:
/// ```text
/// 'data.frame':  N obs. of  M variables:
///  $ col1: int  1 2 3 ...
///  $ col2: chr  "a" "b" "c" ...
/// ```
pub(crate) fn str_data_frame(val: &RValue) -> Option<String> {
    let data = extract_data_frame(val)?;

    let mut out = String::new();
    out.push_str(&format!(
        "'data.frame':\t{} obs. of  {} variables:\n",
        data.nrow,
        data.col_names.len()
    ));

    let max_preview = 10;

    // Build rows for each column: "$ name : type  preview..."
    let mut builder = Builder::new();

    for (i, col_name) in data.col_names.iter().enumerate() {
        let ty = data.col_types[i];
        let short_ty = short_type_name(ty);

        // Build preview of first N elements
        let preview: String = data.columns[i]
            .iter()
            .take(max_preview)
            .map(|val| {
                if ty == "character" {
                    format!("\"{}\"", val)
                } else {
                    val.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let ellipsis = if data.nrow > max_preview { " ..." } else { "" };

        builder.push_record([
            format!(" $ {}", col_name),
            format!(": {}", short_ty),
            format!(" {}{}", preview, ellipsis),
        ]);
    }

    let mut table = builder.build();
    table.with(Style::empty());

    out.push_str(&table.to_string());
    Some(out)
}

/// Map R type names to short abbreviations for display.
fn short_type_name(ty: &str) -> &str {
    match ty {
        "integer" => "int",
        "double" => "num",
        "character" => "chr",
        "logical" => "lgl",
        "complex" => "cpl",
        "raw" => "raw",
        "NULL" => "NULL",
        "list" => "list",
        _ => ty,
    }
}

// endregion
