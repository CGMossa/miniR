//! TOML builtins — `read.toml()`, `write.toml()`, `toml_parse()`, and `toml_serialize()`
//! for reading, writing, and converting between R values and TOML.

use std::collections::HashSet;

use super::CallArgs;
use crate::interpreter::value::*;
use minir_macros::builtin;

// region: toml_parse

/// Parse a TOML string into an R value.
///
/// Conversion rules:
/// - TOML table -> named list
/// - TOML array of tables (homogeneous keys) -> data.frame
/// - TOML array of scalars -> atomic vector
/// - TOML array of mixed types -> list
/// - TOML string -> character
/// - TOML integer -> integer
/// - TOML float -> double
/// - TOML boolean -> logical
/// - TOML datetime -> character (ISO 8601 string)
///
/// @param text character scalar: TOML string to parse
/// @return R value corresponding to the TOML structure
#[builtin(name = "toml_parse", min_args = 1, namespace = "utils")]
fn builtin_toml_parse(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let text = call_args.string("text", 0)?;

    let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        RError::new(RErrorKind::Other, format!("TOML parse error: {e}"))
    })?;

    table_to_rvalue(doc.as_table())
}

// endregion

// region: toml_serialize

/// Convert an R value to a TOML string.
///
/// Conversion rules:
/// - Named list -> TOML table
/// - Atomic vector of length 1 -> TOML scalar
/// - Atomic vector of length > 1 -> TOML array
/// - NULL -> omitted
///
/// @param x R value to convert (typically a named list)
/// @return character scalar containing the TOML string
#[builtin(name = "toml_serialize", min_args = 1, namespace = "utils")]
fn builtin_toml_serialize(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let value = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    let RValue::List(list) = value else {
        return Err(RError::new(
            RErrorKind::Type,
            "toml_serialize() requires a named list as input".to_string(),
        ));
    };

    let table = rlist_to_table(list)?;
    let mut doc = toml_edit::DocumentMut::new();
    // Copy all items from our table into the document
    for (key, item) in table.iter() {
        doc.insert(key, item.clone());
    }

    Ok(RValue::vec(Vector::Character(
        vec![Some(doc.to_string())].into(),
    )))
}

// endregion

// region: read.toml

/// Read a TOML file and return its contents as an R named list.
///
/// @param file character scalar: path to the TOML file
/// @return named list representing the TOML document
#[builtin(name = "read.toml", min_args = 1, namespace = "utils")]
fn builtin_read_toml(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let path = call_args.string("file", 0)?;

    let content = std::fs::read_to_string(&path).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot open file '{}': {}", path, e),
        )
    })?;

    let doc: toml_edit::DocumentMut = content.parse().map_err(|e: toml_edit::TomlError| {
        RError::new(
            RErrorKind::Other,
            format!("TOML parse error in '{}': {}", path, e),
        )
    })?;

    table_to_rvalue(doc.as_table())
}

// endregion

// region: write.toml

/// Write an R named list as a TOML file.
///
/// @param x named list to serialize
/// @param file character scalar: path to the output TOML file
/// @return NULL (invisibly)
#[builtin(name = "write.toml", min_args = 2, namespace = "utils")]
fn builtin_write_toml(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let value = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;
    let path = call_args.string("file", 1)?;

    let RValue::List(list) = value else {
        return Err(RError::new(
            RErrorKind::Type,
            "write.toml() requires a named list as input".to_string(),
        ));
    };

    let table = rlist_to_table(list)?;
    let mut doc = toml_edit::DocumentMut::new();
    for (key, item) in table.iter() {
        doc.insert(key, item.clone());
    }

    std::fs::write(&path, doc.to_string()).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot write to file '{}': {}", path, e),
        )
    })?;

    Ok(RValue::Null)
}

// endregion

// region: TOML -> RValue conversion

/// Convert a TOML table to an R named list.
fn table_to_rvalue(table: &toml_edit::Table) -> Result<RValue, RError> {
    let mut entries: Vec<(Option<String>, RValue)> = Vec::with_capacity(table.len());
    for (key, item) in table.iter() {
        entries.push((Some(key.to_string()), item_to_rvalue(item)?));
    }
    Ok(RValue::List(RList::new(entries)))
}

/// Convert a TOML `Item` to an `RValue`.
fn item_to_rvalue(item: &toml_edit::Item) -> Result<RValue, RError> {
    match item {
        toml_edit::Item::None => Ok(RValue::Null),
        toml_edit::Item::Value(v) => value_to_rvalue(v),
        toml_edit::Item::Table(t) => table_to_rvalue(t),
        toml_edit::Item::ArrayOfTables(aot) => array_of_tables_to_rvalue(aot),
    }
}

/// Convert a TOML `Value` to an `RValue`.
fn value_to_rvalue(value: &toml_edit::Value) -> Result<RValue, RError> {
    match value {
        toml_edit::Value::String(s) => Ok(RValue::vec(Vector::Character(
            vec![Some(s.value().to_string())].into(),
        ))),
        toml_edit::Value::Integer(i) => {
            Ok(RValue::vec(Vector::Integer(vec![Some(*i.value())].into())))
        }
        toml_edit::Value::Float(f) => {
            Ok(RValue::vec(Vector::Double(vec![Some(*f.value())].into())))
        }
        toml_edit::Value::Boolean(b) => {
            Ok(RValue::vec(Vector::Logical(vec![Some(*b.value())].into())))
        }
        toml_edit::Value::Datetime(dt) => {
            // Convert TOML datetime to its string representation
            Ok(RValue::vec(Vector::Character(
                vec![Some(dt.value().to_string())].into(),
            )))
        }
        toml_edit::Value::Array(arr) => toml_array_to_rvalue(arr),
        toml_edit::Value::InlineTable(t) => inline_table_to_rvalue(t),
    }
}

/// Convert a TOML inline table to an R named list.
fn inline_table_to_rvalue(table: &toml_edit::InlineTable) -> Result<RValue, RError> {
    let mut entries: Vec<(Option<String>, RValue)> = Vec::with_capacity(table.len());
    for (key, value) in table.iter() {
        entries.push((Some(key.to_string()), value_to_rvalue(value)?));
    }
    Ok(RValue::List(RList::new(entries)))
}

/// Convert a TOML array to an R value.
///
/// If all elements are scalars of the same type, produce an atomic vector.
/// If all elements are tables with the same keys, produce a data.frame.
/// Otherwise, produce a list.
fn toml_array_to_rvalue(arr: &toml_edit::Array) -> Result<RValue, RError> {
    let items: Vec<&toml_edit::Value> = arr.iter().collect();

    if items.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Try homogeneous scalar array -> atomic vector
    if let Some(vec) = try_toml_array_as_vector(&items) {
        return Ok(vec);
    }

    // Try array of inline tables with same keys -> data.frame
    if let Some(df) = try_toml_array_as_dataframe(&items)? {
        return Ok(df);
    }

    // Fallback: heterogeneous list
    let elements: Result<Vec<(Option<String>, RValue)>, RError> = items
        .iter()
        .map(|v| Ok((None, value_to_rvalue(v)?)))
        .collect();
    Ok(RValue::List(RList::new(elements?)))
}

/// Try to convert a TOML array of scalars into an atomic vector.
/// Returns `None` if the array contains non-scalar values or mixed types.
fn try_toml_array_as_vector(items: &[&toml_edit::Value]) -> Option<RValue> {
    // Check what scalar types are present
    let mut has_string = false;
    let mut has_int = false;
    let mut has_float = false;
    let mut has_bool = false;
    let mut has_datetime = false;
    let mut has_non_scalar = false;

    for item in items {
        match item {
            toml_edit::Value::String(_) => has_string = true,
            toml_edit::Value::Integer(_) => has_int = true,
            toml_edit::Value::Float(_) => has_float = true,
            toml_edit::Value::Boolean(_) => has_bool = true,
            toml_edit::Value::Datetime(_) => has_datetime = true,
            toml_edit::Value::Array(_) | toml_edit::Value::InlineTable(_) => {
                has_non_scalar = true;
            }
        }
    }

    if has_non_scalar {
        return None;
    }

    // Datetimes go to character
    if has_datetime {
        let result: Vec<Option<String>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::Datetime(dt) => Some(dt.value().to_string()),
                toml_edit::Value::String(s) => Some(s.value().to_string()),
                _ => Some(format!("{}", v)),
            })
            .collect();
        return Some(RValue::vec(Vector::Character(result.into())));
    }

    // Strings dominate
    if has_string {
        let result: Vec<Option<String>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::String(s) => Some(s.value().to_string()),
                toml_edit::Value::Boolean(b) => {
                    Some(if *b.value() { "TRUE" } else { "FALSE" }.to_string())
                }
                toml_edit::Value::Integer(i) => Some(i.value().to_string()),
                toml_edit::Value::Float(f) => Some(f.value().to_string()),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Character(result.into())));
    }

    // Pure booleans (no numbers)
    if has_bool && !has_int && !has_float {
        let result: Vec<Option<bool>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::Boolean(b) => Some(*b.value()),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Logical(result.into())));
    }

    // Has floats -> all numeric becomes double
    if has_float {
        let result: Vec<Option<f64>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::Float(f) => Some(*f.value()),
                toml_edit::Value::Integer(i) => {
                    // Safe: i64 always fits in f64 (may lose precision for very large values)
                    #[allow(clippy::cast_precision_loss)]
                    Some(*i.value() as f64)
                }
                toml_edit::Value::Boolean(b) => Some(if *b.value() { 1.0 } else { 0.0 }),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Double(result.into())));
    }

    // Pure integers
    if has_int {
        let result: Vec<Option<i64>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::Integer(i) => Some(*i.value()),
                toml_edit::Value::Boolean(b) => Some(i64::from(*b.value())),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Integer(result.into())));
    }

    // Pure booleans with numbers -> integer
    if has_bool {
        let result: Vec<Option<i64>> = items
            .iter()
            .map(|v| match v {
                toml_edit::Value::Boolean(b) => Some(i64::from(*b.value())),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Integer(result.into())));
    }

    None
}

/// Try to convert a TOML array of inline tables into a data.frame.
/// Returns `None` if the elements are not all inline tables with the same keys.
fn try_toml_array_as_dataframe(items: &[&toml_edit::Value]) -> Result<Option<RValue>, RError> {
    // Check all elements are inline tables
    let mut tables: Vec<&toml_edit::InlineTable> = Vec::with_capacity(items.len());
    for item in items {
        if let toml_edit::Value::InlineTable(t) = item {
            tables.push(t);
        } else {
            return Ok(None);
        }
    }

    if tables.is_empty() {
        return Ok(None);
    }

    // Check all tables have the same keys
    let first_keys: HashSet<&str> = tables[0].iter().map(|(k, _)| k).collect();
    for table in &tables[1..] {
        let keys: HashSet<&str> = table.iter().map(|(k, _)| k).collect();
        if keys != first_keys {
            return Ok(None);
        }
    }

    // Collect column names in order from first table
    let col_names: Vec<String> = tables[0].iter().map(|(k, _)| k.to_string()).collect();
    let nrows = tables.len();

    // Build each column as an R vector
    let mut list_cols: Vec<(Option<String>, RValue)> = Vec::new();
    for col_name in &col_names {
        let vals: Vec<&toml_edit::Value> = tables
            .iter()
            .map(|t| {
                t.get(col_name.as_str())
                    .expect("key verified to exist in all tables")
            })
            .collect();
        let col_value = coerce_toml_column(&vals)?;
        list_cols.push((Some(col_name.clone()), col_value));
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
    let row_names: Vec<Option<i64>> = (1..=i64::try_from(nrows)?).map(Some).collect();
    list.set_attr(
        "row.names".to_string(),
        RValue::vec(Vector::Integer(row_names.into())),
    );

    Ok(Some(RValue::List(list)))
}

/// Convert an array of tables (TOML `[[section]]`) to an R value.
///
/// If all tables have the same keys, produce a data.frame.
/// Otherwise, produce a list.
fn array_of_tables_to_rvalue(aot: &toml_edit::ArrayOfTables) -> Result<RValue, RError> {
    let tables: Vec<&toml_edit::Table> = aot.iter().collect();

    if tables.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Check if all tables have the same keys -> data.frame
    let first_keys: HashSet<&str> = tables[0].iter().map(|(k, _)| k).collect();
    let all_same_keys = tables[1..]
        .iter()
        .all(|t| t.iter().map(|(k, _)| k).collect::<HashSet<_>>() == first_keys);

    if all_same_keys && !first_keys.is_empty() {
        let col_names: Vec<String> = tables[0].iter().map(|(k, _)| k.to_string()).collect();
        let nrows = tables.len();

        let mut list_cols: Vec<(Option<String>, RValue)> = Vec::new();
        for col_name in &col_names {
            let vals: Vec<&toml_edit::Item> = tables
                .iter()
                .map(|t| {
                    t.get(col_name.as_str())
                        .expect("key verified to exist in all tables")
                })
                .collect();
            let col_value = coerce_toml_item_column(&vals)?;
            list_cols.push((Some(col_name.clone()), col_value));
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
        let row_names: Vec<Option<i64>> = (1..=i64::try_from(nrows)?).map(Some).collect();
        list.set_attr(
            "row.names".to_string(),
            RValue::vec(Vector::Integer(row_names.into())),
        );

        return Ok(RValue::List(list));
    }

    // Fallback: list of named lists
    let elements: Result<Vec<(Option<String>, RValue)>, RError> = tables
        .iter()
        .map(|t| Ok((None, table_to_rvalue(t)?)))
        .collect();
    Ok(RValue::List(RList::new(elements?)))
}

/// Coerce a column of TOML values to an R vector (for data.frame construction).
fn coerce_toml_column(vals: &[&toml_edit::Value]) -> Result<RValue, RError> {
    let mut has_string = false;
    let mut has_int = false;
    let mut has_float = false;
    let mut has_bool = false;
    let mut has_datetime = false;
    let mut has_complex = false; // arrays/tables

    for v in vals {
        match v {
            toml_edit::Value::String(_) => has_string = true,
            toml_edit::Value::Integer(_) => has_int = true,
            toml_edit::Value::Float(_) => has_float = true,
            toml_edit::Value::Boolean(_) => has_bool = true,
            toml_edit::Value::Datetime(_) => has_datetime = true,
            _ => has_complex = true,
        }
    }

    // If complex values present, fall back to list column
    if has_complex {
        let elements: Result<Vec<(Option<String>, RValue)>, RError> = vals
            .iter()
            .map(|v| Ok((None, value_to_rvalue(v)?)))
            .collect();
        return Ok(RValue::List(RList::new(elements?)));
    }

    // Datetimes and strings -> character
    if has_string || has_datetime {
        let result: Vec<Option<String>> = vals
            .iter()
            .map(|v| match v {
                toml_edit::Value::String(s) => Some(s.value().to_string()),
                toml_edit::Value::Datetime(dt) => Some(dt.value().to_string()),
                toml_edit::Value::Boolean(b) => {
                    Some(if *b.value() { "TRUE" } else { "FALSE" }.to_string())
                }
                toml_edit::Value::Integer(i) => Some(i.value().to_string()),
                toml_edit::Value::Float(f) => Some(f.value().to_string()),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Character(result.into())));
    }

    // Pure booleans
    if has_bool && !has_int && !has_float {
        let result: Vec<Option<bool>> = vals
            .iter()
            .map(|v| match v {
                toml_edit::Value::Boolean(b) => Some(*b.value()),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    // Has floats -> double
    if has_float {
        let result: Vec<Option<f64>> = vals
            .iter()
            .map(|v| match v {
                toml_edit::Value::Float(f) => Some(*f.value()),
                toml_edit::Value::Integer(i) =>
                {
                    #[allow(clippy::cast_precision_loss)]
                    Some(*i.value() as f64)
                }
                toml_edit::Value::Boolean(b) => Some(if *b.value() { 1.0 } else { 0.0 }),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Double(result.into())));
    }

    // Pure integers
    if has_int {
        let result: Vec<Option<i64>> = vals
            .iter()
            .map(|v| match v {
                toml_edit::Value::Integer(i) => Some(*i.value()),
                toml_edit::Value::Boolean(b) => Some(i64::from(*b.value())),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(result.into())));
    }

    // Pure booleans with numbers
    if has_bool {
        let result: Vec<Option<i64>> = vals
            .iter()
            .map(|v| match v {
                toml_edit::Value::Boolean(b) => Some(i64::from(*b.value())),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(result.into())));
    }

    // All empty? Return empty logical
    Ok(RValue::vec(Vector::Logical(
        Vec::<Option<bool>>::new().into(),
    )))
}

/// Coerce a column of TOML Items to an R vector (for array-of-tables data.frame).
fn coerce_toml_item_column(vals: &[&toml_edit::Item]) -> Result<RValue, RError> {
    // Extract Values from Items, falling back to list for non-Value items
    let mut values: Vec<&toml_edit::Value> = Vec::with_capacity(vals.len());
    let mut has_non_value = false;

    for item in vals {
        if let Some(v) = item.as_value() {
            values.push(v);
        } else {
            has_non_value = true;
            break;
        }
    }

    if has_non_value {
        // Fall back to list of items
        let elements: Result<Vec<(Option<String>, RValue)>, RError> = vals
            .iter()
            .map(|item| Ok((None, item_to_rvalue(item)?)))
            .collect();
        return Ok(RValue::List(RList::new(elements?)));
    }

    coerce_toml_column(&values)
}

// endregion

// region: RValue -> TOML conversion

/// Convert an R named list to a TOML `Table`.
fn rlist_to_table(list: &RList) -> Result<toml_edit::Table, RError> {
    let mut table = toml_edit::Table::new();

    for (name, value) in &list.values {
        let key = name.as_ref().ok_or_else(|| {
            RError::new(
                RErrorKind::Type,
                "TOML requires all list elements to be named".to_string(),
            )
        })?;

        match value {
            RValue::Null => {
                // TOML has no null — skip null entries
            }
            RValue::Vector(rv) => {
                table.insert(key, toml_edit::Item::Value(vector_to_toml(&rv.inner)?));
            }
            RValue::List(inner_list) => {
                // Check if this is a nested table or should be an inline table
                if is_simple_list(inner_list) {
                    table.insert(
                        key,
                        toml_edit::Item::Value(toml_edit::Value::InlineTable(
                            rlist_to_inline_table(inner_list)?,
                        )),
                    );
                } else {
                    let subtable = rlist_to_table(inner_list)?;
                    table.insert(key, toml_edit::Item::Table(subtable));
                }
            }
            RValue::Function(_) => {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!("cannot convert function to TOML (key '{key}')"),
                ));
            }
            RValue::Environment(_) => {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!("cannot convert environment to TOML (key '{key}')"),
                ));
            }
            RValue::Language(_) => {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!("cannot convert language object to TOML (key '{key}')"),
                ));
            }
            RValue::Promise(_) => {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!("cannot convert promise to TOML (key '{key}') — force it first"),
                ));
            }
        }
    }

    Ok(table)
}

/// Check if a list is "simple" (all scalar values, no nested lists/tables).
/// Simple lists become inline tables, complex ones become regular tables.
fn is_simple_list(list: &RList) -> bool {
    list.values.iter().all(|(_, v)| match v {
        RValue::Vector(rv) => rv.inner.len() <= 1,
        RValue::Null => true,
        _ => false,
    })
}

/// Convert an R named list to a TOML inline table.
fn rlist_to_inline_table(list: &RList) -> Result<toml_edit::InlineTable, RError> {
    let mut table = toml_edit::InlineTable::new();

    for (name, value) in &list.values {
        let key = name.as_ref().ok_or_else(|| {
            RError::new(
                RErrorKind::Type,
                "TOML requires all list elements to be named".to_string(),
            )
        })?;

        match value {
            RValue::Null => {}
            RValue::Vector(rv) => {
                table.insert(key, vector_to_toml(&rv.inner)?);
            }
            RValue::List(inner) => {
                table.insert(
                    key,
                    toml_edit::Value::InlineTable(rlist_to_inline_table(inner)?),
                );
            }
            _ => {
                return Err(RError::new(
                    RErrorKind::Type,
                    format!("cannot convert {} to TOML (key '{key}')", value.type_name()),
                ));
            }
        }
    }

    Ok(table)
}

/// Convert an R atomic vector to a TOML `Value`.
/// Scalars (length 1) become TOML scalars; longer vectors become TOML arrays.
fn vector_to_toml(vec: &Vector) -> Result<toml_edit::Value, RError> {
    match vec {
        Vector::Logical(v) => {
            if v.len() == 1 {
                match v[0] {
                    Some(b) => Ok(toml_edit::Value::from(b)),
                    None => Err(RError::new(
                        RErrorKind::Type,
                        "TOML does not support NA values".to_string(),
                    )),
                }
            } else {
                let mut arr = toml_edit::Array::new();
                for item in v.iter() {
                    match item {
                        Some(b) => arr.push_formatted(toml_edit::Value::from(*b)),
                        None => {
                            return Err(RError::new(
                                RErrorKind::Type,
                                "TOML does not support NA values".to_string(),
                            ))
                        }
                    }
                }
                Ok(toml_edit::Value::Array(arr))
            }
        }
        Vector::Integer(v) => {
            if v.len() == 1 {
                match v.get_opt(0) {
                    Some(i) => Ok(toml_edit::Value::from(i)),
                    None => Err(RError::new(
                        RErrorKind::Type,
                        "TOML does not support NA values".to_string(),
                    )),
                }
            } else {
                let mut arr = toml_edit::Array::new();
                for item in v.iter_opt() {
                    match item {
                        Some(i) => arr.push_formatted(toml_edit::Value::from(i)),
                        None => {
                            return Err(RError::new(
                                RErrorKind::Type,
                                "TOML does not support NA values".to_string(),
                            ))
                        }
                    }
                }
                Ok(toml_edit::Value::Array(arr))
            }
        }
        Vector::Double(v) => {
            if v.len() == 1 {
                match v.get_opt(0) {
                    Some(f) => double_to_toml(f),
                    None => Err(RError::new(
                        RErrorKind::Type,
                        "TOML does not support NA values".to_string(),
                    )),
                }
            } else {
                let mut arr = toml_edit::Array::new();
                for item in v.iter_opt() {
                    match item {
                        Some(f) => arr.push_formatted(double_to_toml(f)?),
                        None => {
                            return Err(RError::new(
                                RErrorKind::Type,
                                "TOML does not support NA values".to_string(),
                            ))
                        }
                    }
                }
                Ok(toml_edit::Value::Array(arr))
            }
        }
        Vector::Character(v) => {
            if v.len() == 1 {
                match &v[0] {
                    Some(s) => Ok(toml_edit::Value::from(s.as_str())),
                    None => Err(RError::new(
                        RErrorKind::Type,
                        "TOML does not support NA values".to_string(),
                    )),
                }
            } else {
                let mut arr = toml_edit::Array::new();
                for item in v.iter() {
                    match item {
                        Some(s) => arr.push_formatted(toml_edit::Value::from(s.as_str())),
                        None => {
                            return Err(RError::new(
                                RErrorKind::Type,
                                "TOML does not support NA values".to_string(),
                            ))
                        }
                    }
                }
                Ok(toml_edit::Value::Array(arr))
            }
        }
        Vector::Complex(_) => Err(RError::new(
            RErrorKind::Type,
            "cannot convert complex numbers to TOML".to_string(),
        )),
        Vector::Raw(_) => Err(RError::new(
            RErrorKind::Type,
            "cannot convert raw bytes to TOML".to_string(),
        )),
    }
}

/// Convert an f64 to a TOML value, handling special float values.
fn double_to_toml(f: f64) -> Result<toml_edit::Value, RError> {
    if f.is_nan() {
        Ok(toml_edit::Value::from(f64::NAN))
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            Ok(toml_edit::Value::from(f64::INFINITY))
        } else {
            Ok(toml_edit::Value::from(f64::NEG_INFINITY))
        }
    } else {
        Ok(toml_edit::Value::from(f))
    }
}

// endregion
