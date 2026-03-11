//! File I/O builtins — reading and writing data files (CSV, table, lines, scan)
//! and file system utilities (file.path, file.exists).

use crate::interpreter::value::*;
use newr_macros::builtin;

#[builtin]
fn builtin_file_path(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let sep = named
        .iter()
        .find(|(n, _)| n == "fsep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "/".to_string());

    let parts: Vec<String> = args
        .iter()
        .filter_map(|v| v.as_vector()?.as_character_scalar())
        .collect();
    Ok(RValue::vec(Vector::Character(
        vec![Some(parts.join(&sep))].into(),
    )))
}

#[builtin(name = "file.exists", min_args = 1)]
fn builtin_file_exists(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
    let results: Vec<Option<bool>> = args
        .iter()
        .map(|arg| {
            let path = arg
                .as_vector()
                .and_then(|v| v.as_character_scalar())
                .unwrap_or_default();
            Some(std::path::Path::new(&path).exists())
        })
        .collect();
    Ok(RValue::vec(Vector::Logical(results.into())))
}

#[builtin(name = "readLines", min_args = 1)]
fn builtin_read_lines(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("invalid 'con' argument".to_string()))?;
    let n = named
        .iter()
        .find(|(n, _)| n == "n")
        .or_else(|| named.iter().find(|(n, _)| n == "n"))
        .and_then(|(_, v)| v.as_vector()?.as_integer_scalar())
        .unwrap_or(-1);

    let content = std::fs::read_to_string(&path)
        .map_err(|e| RError::Other(format!("cannot open connection: {}", e)))?;
    let lines: Vec<Option<String>> = if n < 0 {
        content.lines().map(|l| Some(l.to_string())).collect()
    } else {
        content
            .lines()
            .take(n as usize)
            .map(|l| Some(l.to_string()))
            .collect()
    };
    Ok(RValue::vec(Vector::Character(lines.into())))
}

#[builtin(name = "writeLines", min_args = 1)]
fn builtin_write_lines(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let text = args
        .first()
        .and_then(|v| v.as_vector())
        .map(|v| v.to_characters())
        .unwrap_or_default();
    let con = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "con").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar());
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .unwrap_or_else(|| "\n".to_string());

    let output: String = text
        .iter()
        .map(|s| s.clone().unwrap_or_else(|| "NA".to_string()))
        .collect::<Vec<_>>()
        .join(&sep);

    match con {
        Some(path) => {
            std::fs::write(&path, format!("{}{}", output, sep))
                .map_err(|e| RError::Other(format!("cannot open connection: {}", e)))?;
        }
        None => {
            // Write to stdout
            println!("{}", output);
        }
    }
    Ok(RValue::Null)
}

#[builtin(name = "read.csv", min_args = 1)]
fn builtin_read_csv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let path = args
        .first()
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("invalid 'file' argument".to_string()))?;

    let header = named
        .iter()
        .find(|(n, _)| n == "header")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| v.as_vector()?.as_character_scalar())
        .and_then(|s| s.bytes().next())
        .unwrap_or(b',');

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(header)
        .delimiter(sep)
        .from_path(&path)
        .map_err(|e| RError::Other(format!("cannot open file '{}': {}", path, e)))?;

    let col_names: Vec<String> = if header {
        rdr.headers()
            .map_err(|e| RError::Other(format!("error reading CSV headers: {}", e)))?
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        // Auto-generate V1, V2, ... column names from first record
        let ncols = rdr
            .records()
            .next()
            .and_then(|r| r.ok())
            .map(|r| r.len())
            .unwrap_or(0);
        (1..=ncols).map(|i| format!("V{}", i)).collect()
    };

    let ncols = col_names.len();
    let mut columns: Vec<Vec<Option<String>>> = vec![vec![]; ncols];
    let mut nrows = 0usize;

    for result in rdr.records() {
        let record =
            result.map_err(|e| RError::Other(format!("error reading CSV record: {}", e)))?;
        for (i, field) in record.iter().enumerate() {
            if i < ncols {
                if field == "NA" || field.is_empty() {
                    columns[i].push(None);
                } else {
                    columns[i].push(Some(field.to_string()));
                }
            }
        }
        nrows += 1;
    }

    // Try to coerce columns to numeric where possible
    let mut list_cols: Vec<(Option<String>, RValue)> = Vec::new();
    for (i, col_data) in columns.into_iter().enumerate() {
        let name = col_names.get(i).cloned();
        // Try parsing all as doubles
        let all_numeric = col_data.iter().all(|v| match v {
            None => true,
            Some(s) => s.parse::<f64>().is_ok(),
        });
        if all_numeric {
            // Try integer first
            let all_int = col_data.iter().all(|v| match v {
                None => true,
                Some(s) => s.parse::<i64>().is_ok(),
            });
            if all_int {
                let vals: Vec<Option<i64>> =
                    col_data.iter().map(|v| v.as_ref()?.parse().ok()).collect();
                list_cols.push((name, RValue::vec(Vector::Integer(vals.into()))));
            } else {
                let vals: Vec<Option<f64>> =
                    col_data.iter().map(|v| v.as_ref()?.parse().ok()).collect();
                list_cols.push((name, RValue::vec(Vector::Double(vals.into()))));
            }
        } else {
            list_cols.push((name, RValue::vec(Vector::Character(col_data.into()))));
        }
    }

    let mut list = RList::new(list_cols);
    list.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("data.frame".to_string())].into(),
        )),
    );
    list.set_attr(
        "names".to_string(),
        RValue::vec(Vector::Character(
            col_names.into_iter().map(Some).collect::<Vec<_>>().into(),
        )),
    );
    let row_names: Vec<Option<i64>> = (1..=nrows as i64).map(Some).collect();
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );
    Ok(RValue::List(list))
}

#[builtin(name = "write.csv", min_args = 1)]
fn builtin_write_csv(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let data = args
        .first()
        .ok_or_else(|| RError::Argument("argument 'x' is missing".to_string()))?;
    let file = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "file").map(|(_, v)| v))
        .and_then(|v| v.as_vector()?.as_character_scalar())
        .ok_or_else(|| RError::Argument("argument 'file' is missing".to_string()))?;

    let row_names = named
        .iter()
        .find(|(n, _)| n == "row.names")
        .and_then(|(_, v)| v.as_vector()?.as_logical_scalar())
        .unwrap_or(true);

    let RValue::List(list) = data else {
        return Err(RError::Argument(
            "write.csv requires a data frame or list".to_string(),
        ));
    };

    let mut wtr = csv::Writer::from_path(&file)
        .map_err(|e| RError::Other(format!("cannot open file '{}': {}", file, e)))?;

    // Write header
    let col_names: Vec<String> = list
        .values
        .iter()
        .map(|(n, _)| n.clone().unwrap_or_default())
        .collect();

    if row_names {
        let mut header = vec!["".to_string()];
        header.extend(col_names.clone());
        wtr.write_record(&header)
            .map_err(|e| RError::Other(format!("error writing CSV: {}", e)))?;
    } else {
        wtr.write_record(&col_names)
            .map_err(|e| RError::Other(format!("error writing CSV: {}", e)))?;
    }

    // Determine number of rows
    let nrows = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

    // Write rows
    for row in 0..nrows {
        let mut record: Vec<String> = Vec::new();
        if row_names {
            record.push((row + 1).to_string());
        }
        for (_, col_val) in &list.values {
            if let RValue::Vector(rv) = col_val {
                let chars = rv.to_characters();
                record.push(
                    chars
                        .get(row)
                        .and_then(|v| v.clone())
                        .unwrap_or_else(|| "NA".to_string()),
                );
            } else {
                record.push("NA".to_string());
            }
        }
        wtr.write_record(&record)
            .map_err(|e| RError::Other(format!("error writing CSV: {}", e)))?;
    }

    wtr.flush()
        .map_err(|e| RError::Other(format!("error flushing CSV: {}", e)))?;
    Ok(RValue::Null)
}

#[builtin]
fn builtin_scan(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = args
        .first()
        .and_then(|v| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_default();

    if file.is_empty() {
        return Err(RError::Argument(
            "scan() requires a file path — reading from stdin is not yet supported".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&file)
        .map_err(|e| RError::Other(format!("cannot open file '{}': {}", file, e)))?;

    // Determine separator
    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        });

    let tokens: Vec<&str> = match &sep {
        Some(s) if !s.is_empty() => content.split(s.as_str()).collect(),
        _ => content.split_whitespace().collect(),
    };

    // Determine what type to return (default: character)
    let what = args
        .get(1)
        .or_else(|| named.iter().find(|(n, _)| n == "what").map(|(_, v)| v));

    match what {
        Some(RValue::Vector(rv)) => match &rv.inner {
            Vector::Double(_) => {
                let vals: Vec<Option<f64>> = tokens.iter().map(|t| t.parse::<f64>().ok()).collect();
                Ok(RValue::vec(Vector::Double(vals.into())))
            }
            Vector::Integer(_) => {
                let vals: Vec<Option<i64>> = tokens.iter().map(|t| t.parse::<i64>().ok()).collect();
                Ok(RValue::vec(Vector::Integer(vals.into())))
            }
            Vector::Logical(_) => {
                let vals: Vec<Option<bool>> = tokens
                    .iter()
                    .map(|t| match *t {
                        "TRUE" | "T" => Some(true),
                        "FALSE" | "F" => Some(false),
                        _ => None,
                    })
                    .collect();
                Ok(RValue::vec(Vector::Logical(vals.into())))
            }
            _ => {
                let vals: Vec<Option<String>> =
                    tokens.iter().map(|t| Some(t.to_string())).collect();
                Ok(RValue::vec(Vector::Character(vals.into())))
            }
        },
        _ => {
            let vals: Vec<Option<String>> = tokens.iter().map(|t| Some(t.to_string())).collect();
            Ok(RValue::vec(Vector::Character(vals.into())))
        }
    }
}

#[builtin(name = "read.table", min_args = 1)]
fn builtin_read_table(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = match &args[0] {
        RValue::Vector(rv) => rv.inner.as_character_scalar().ok_or_else(|| {
            RError::Argument("read.table() requires a file path string".to_string())
        })?,
        _ => {
            return Err(RError::Argument(
                "read.table() requires a file path string".to_string(),
            ))
        }
    };

    let header = named
        .iter()
        .find(|(n, _)| n == "header")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(false);

    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_else(|| "".to_string()); // empty = whitespace

    let content = std::fs::read_to_string(&file)
        .map_err(|e| RError::Other(format!("cannot open file '{}': {}", file, e)))?;

    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Parse column names from header
    let col_names: Vec<String> = if header {
        let header_line = lines.remove(0);
        split_line(header_line, &sep)
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    };

    // Parse data
    let rows: Vec<Vec<String>> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| split_line(l, &sep).iter().map(|s| s.to_string()).collect())
        .collect();

    if rows.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut columns: Vec<(Option<String>, RValue)> = Vec::with_capacity(ncols);

    for col_idx in 0..ncols {
        let col_data: Vec<Option<String>> = rows.iter().map(|r| r.get(col_idx).cloned()).collect();

        // Try to detect numeric columns
        let all_numeric = col_data.iter().all(|v| {
            v.as_ref()
                .is_none_or(|s| s.is_empty() || s == "NA" || s.parse::<f64>().is_ok())
        });

        let col_val = if all_numeric {
            let vals: Vec<Option<f64>> = col_data
                .iter()
                .map(|v| {
                    v.as_ref().and_then(|s| {
                        if s == "NA" || s.is_empty() {
                            None
                        } else {
                            s.parse().ok()
                        }
                    })
                })
                .collect();
            RValue::vec(Vector::Double(vals.into()))
        } else {
            RValue::vec(Vector::Character(col_data.into()))
        };

        let name = col_names
            .get(col_idx)
            .cloned()
            .or_else(|| Some(format!("V{}", col_idx + 1)));
        columns.push((name, col_val));
    }

    Ok(RValue::List(RList::new(columns)))
}

#[builtin(name = "write.table", min_args = 2)]
fn builtin_write_table(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let file = match &args[1] {
        RValue::Vector(rv) => rv
            .inner
            .as_character_scalar()
            .ok_or_else(|| RError::Argument("write.table() requires a file path".to_string()))?,
        _ => {
            return Err(RError::Argument(
                "write.table() requires a file path as second argument".to_string(),
            ))
        }
    };

    let sep = named
        .iter()
        .find(|(n, _)| n == "sep")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_character_scalar(),
            _ => None,
        })
        .unwrap_or_else(|| " ".to_string());

    let col_names = named
        .iter()
        .find(|(n, _)| n == "col.names")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(true);

    let quote = named
        .iter()
        .find(|(n, _)| n == "quote")
        .and_then(|(_, v)| match v {
            RValue::Vector(rv) => rv.inner.as_logical_scalar(),
            _ => None,
        })
        .unwrap_or(true);

    let mut output = String::new();

    match &args[0] {
        RValue::List(list) => {
            let ncols = list.values.len();
            let nrows = list.values.first().map(|(_, v)| v.length()).unwrap_or(0);

            // Header
            if col_names {
                let names: Vec<String> = list
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| {
                        let n = name.clone().unwrap_or_else(|| format!("V{}", i + 1));
                        if quote {
                            format!("\"{}\"", n)
                        } else {
                            n
                        }
                    })
                    .collect();
                output.push_str(&names.join(&sep));
                output.push('\n');
            }

            // Rows
            for row_idx in 0..nrows {
                let cells: Vec<String> = (0..ncols)
                    .map(|col_idx| {
                        let (_, val) = &list.values[col_idx];
                        format_cell(val, row_idx, quote)
                    })
                    .collect();
                output.push_str(&cells.join(&sep));
                output.push('\n');
            }
        }
        RValue::Vector(rv) => {
            // Matrix — write rows
            let dim = rv.get_attr("dim");
            match dim {
                Some(RValue::Vector(dim_rv)) => {
                    if let Vector::Integer(d) = &dim_rv.inner {
                        if d.len() >= 2 {
                            let nrow = d[0].unwrap_or(0) as usize;
                            let ncol = d[1].unwrap_or(0) as usize;
                            for r in 0..nrow {
                                let cells: Vec<String> = (0..ncol)
                                    .map(|c| {
                                        let idx = c * nrow + r;
                                        format_cell(&args[0], idx, quote)
                                    })
                                    .collect();
                                output.push_str(&cells.join(&sep));
                                output.push('\n');
                            }
                        }
                    }
                }
                _ => {
                    // Plain vector — one element per line
                    for i in 0..rv.inner.len() {
                        output.push_str(&format_cell(&args[0], i, quote));
                        output.push('\n');
                    }
                }
            }
        }
        _ => {
            return Err(RError::Type(
                "write.table() requires a list or matrix".to_string(),
            ))
        }
    }

    std::fs::write(&file, output)
        .map_err(|e| RError::Other(format!("cannot write to file '{}': {}", file, e)))?;

    Ok(RValue::Null)
}

/// Split a line by separator (whitespace if empty).
fn split_line<'a>(line: &'a str, sep: &str) -> Vec<&'a str> {
    if sep.is_empty() {
        line.split_whitespace().collect()
    } else {
        line.split(sep).collect()
    }
}

/// Format a single cell from a vector for write.table output.
fn format_cell(val: &RValue, idx: usize, quote: bool) -> String {
    match val {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Double(v) => v
                .get(idx)
                .and_then(|x| *x)
                .map_or("NA".to_string(), |f| format!("{}", f)),
            Vector::Integer(v) => v
                .get(idx)
                .and_then(|x| *x)
                .map_or("NA".to_string(), |i| format!("{}", i)),
            Vector::Logical(v) => v.get(idx).and_then(|x| *x).map_or("NA".to_string(), |b| {
                if b { "TRUE" } else { "FALSE" }.to_string()
            }),
            Vector::Character(v) => {
                v.get(idx)
                    .and_then(|x| x.as_ref())
                    .map_or("NA".to_string(), |s| {
                        if quote {
                            format!("\"{}\"", s)
                        } else {
                            s.clone()
                        }
                    })
            }
        },
        _ => "NA".to_string(),
    }
}
