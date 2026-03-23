//! CSV drag-and-drop: parse a CSV/TSV file into `TableData` for View() display.
//!
//! Feature-gated on `plot` + `io` since it needs both the GUI window and the
//! `csv` crate.

use super::view::{ColType, TableData};
use std::path::Path;

/// Read a CSV or TSV file and convert it into `TableData` for the egui viewer.
///
/// - Headers become `TableData.headers`.
/// - Each record becomes a row in `TableData.rows`.
/// - Column types are inferred from the first non-empty value in each column.
/// - Row names are sequential integers ("1", "2", "3", ...).
/// - The tab title is the filename stem.
pub fn csv_to_table_data(path: &Path) -> Result<TableData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let delimiter: u8 = match ext.as_str() {
        "tsv" | "tab" => b'\t',
        _ => b',',
    };

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(delimiter)
        .from_path(path)
        .map_err(|e| format!("failed to open '{}': {}", path.display(), e))?;

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("failed to read CSV headers: {}", e))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let ncols = headers.len();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for (row_idx, result) in rdr.records().enumerate() {
        let record = result.map_err(|e| format!("error reading row {}: {}", row_idx + 1, e))?;
        let mut row = Vec::with_capacity(ncols);
        for col_idx in 0..ncols {
            let field = record.get(col_idx).unwrap_or("");
            row.push(field.to_string());
        }
        rows.push(row);
    }

    let col_types = infer_col_types(&headers, &rows);

    let row_names: Vec<String> = (1..=rows.len()).map(|i| i.to_string()).collect();

    let title = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("CSV")
        .to_string();

    Ok(TableData {
        title,
        headers,
        col_types,
        rows,
        row_names,
    })
}

/// Infer the `ColType` for each column by examining the first non-empty value.
fn infer_col_types(headers: &[String], rows: &[Vec<String>]) -> Vec<ColType> {
    let ncols = headers.len();
    let mut types = vec![ColType::Character; ncols];

    for (col, col_type) in types.iter_mut().enumerate() {
        // Find first non-empty, non-NA value
        let sample = rows.iter().find_map(|row| {
            let val = row.get(col).map(|s| s.as_str()).unwrap_or("");
            if val.is_empty() || val == "NA" {
                None
            } else {
                Some(val)
            }
        });

        if let Some(val) = sample {
            *col_type = infer_single_type(val);
        }
    }

    types
}

/// Infer the type of a single non-empty string value.
fn infer_single_type(val: &str) -> ColType {
    // Check logical first
    let upper = val.to_uppercase();
    if upper == "TRUE" || upper == "FALSE" {
        return ColType::Logical;
    }

    // Try integer (no decimal point)
    if val.parse::<i64>().is_ok() {
        return ColType::Integer;
    }

    // Try double
    if val.parse::<f64>().is_ok() {
        return ColType::Double;
    }

    ColType::Character
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn csv_to_table_data_basic() {
        let dir = temp_dir::TempDir::new().unwrap();
        let csv_path = dir.path().join("test.csv");
        {
            let mut f = std::fs::File::create(&csv_path).unwrap();
            writeln!(f, "name,age,score").unwrap();
            writeln!(f, "Alice,30,95.5").unwrap();
            writeln!(f, "Bob,25,87.0").unwrap();
        }

        let data = csv_to_table_data(&csv_path).unwrap();
        assert_eq!(data.title, "test.csv");
        assert_eq!(data.headers, vec!["name", "age", "score"]);
        assert_eq!(data.rows.len(), 2);
        assert_eq!(data.rows[0], vec!["Alice", "30", "95.5"]);
        assert_eq!(data.rows[1], vec!["Bob", "25", "87.0"]);
        assert_eq!(data.row_names, vec!["1", "2"]);
        assert_eq!(data.col_types[0], ColType::Character);
        assert_eq!(data.col_types[1], ColType::Integer);
        assert_eq!(data.col_types[2], ColType::Double);
    }

    #[test]
    fn tsv_to_table_data() {
        let dir = temp_dir::TempDir::new().unwrap();
        let tsv_path = dir.path().join("data.tsv");
        {
            let mut f = std::fs::File::create(&tsv_path).unwrap();
            writeln!(f, "x\ty\tz").unwrap();
            writeln!(f, "1\tTRUE\thello").unwrap();
        }

        let data = csv_to_table_data(&tsv_path).unwrap();
        assert_eq!(data.headers, vec!["x", "y", "z"]);
        assert_eq!(data.col_types[0], ColType::Integer);
        assert_eq!(data.col_types[1], ColType::Logical);
        assert_eq!(data.col_types[2], ColType::Character);
    }

    #[test]
    fn empty_csv() {
        let dir = temp_dir::TempDir::new().unwrap();
        let csv_path = dir.path().join("empty.csv");
        {
            let mut f = std::fs::File::create(&csv_path).unwrap();
            writeln!(f, "a,b,c").unwrap();
        }

        let data = csv_to_table_data(&csv_path).unwrap();
        assert_eq!(data.headers, vec!["a", "b", "c"]);
        assert!(data.rows.is_empty());
        assert!(data.row_names.is_empty());
    }
}
