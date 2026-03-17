//! JSON builtins — `fromJSON()` and `toJSON()` providing jsonlite-compatible
//! conversion between R values and JSON strings.

use std::collections::HashMap;

use super::CallArgs;
use crate::interpreter::value::*;
use minir_macros::builtin;

// region: fromJSON

/// Parse a JSON string into an R value.
///
/// Conversion rules:
/// - JSON object -> named list
/// - JSON array of objects (same keys) -> data.frame
/// - JSON array of scalars -> vector
/// - JSON null -> NULL
/// - JSON true/false -> logical
/// - JSON number -> double (or integer if representable)
/// - JSON string -> character
///
/// @param txt character scalar: JSON string to parse
/// @return R value corresponding to the JSON structure
#[builtin(name = "fromJSON", min_args = 1, names = ["jsonlite::fromJSON"], namespace = "jsonlite")]
fn builtin_from_json(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let txt = call_args.string("txt", 0)?;

    let json_value: serde_json::Value = serde_json::from_str(&txt)
        .map_err(|e| RError::new(RErrorKind::Other, format!("JSON parse error: {e}")))?;

    json_to_rvalue(&json_value)
}

/// Convert a `serde_json::Value` to an `RValue`.
fn json_to_rvalue(value: &serde_json::Value) -> Result<RValue, RError> {
    match value {
        serde_json::Value::Null => Ok(RValue::Null),
        serde_json::Value::Bool(b) => Ok(RValue::vec(Vector::Logical(vec![Some(*b)].into()))),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                // Check if it fits in i64 (it always does here) and has no fractional part
                Ok(RValue::vec(Vector::Integer(vec![Some(i)].into())))
            } else if let Some(f) = n.as_f64() {
                Ok(RValue::vec(Vector::Double(vec![Some(f)].into())))
            } else {
                Ok(RValue::vec(Vector::Double(vec![None].into())))
            }
        }
        serde_json::Value::String(s) => {
            Ok(RValue::vec(Vector::Character(vec![Some(s.clone())].into())))
        }
        serde_json::Value::Array(arr) => json_array_to_rvalue(arr),
        serde_json::Value::Object(obj) => json_object_to_rvalue(obj),
    }
}

/// Convert a JSON array to an R value.
///
/// If all elements are scalars of the same type, produce an atomic vector.
/// If all elements are objects with the same keys, produce a data.frame.
/// Otherwise, produce a list.
fn json_array_to_rvalue(arr: &[serde_json::Value]) -> Result<RValue, RError> {
    if arr.is_empty() {
        return Ok(RValue::List(RList::new(vec![])));
    }

    // Check if all elements are objects with the same keys -> data.frame
    if let Some(df) = try_array_as_dataframe(arr)? {
        return Ok(df);
    }

    // Check if all elements are homogeneous scalars -> atomic vector
    if let Some(vec) = try_array_as_vector(arr) {
        return Ok(vec);
    }

    // Fallback: heterogeneous list
    let elements: Result<Vec<(Option<String>, RValue)>, RError> =
        arr.iter().map(|v| Ok((None, json_to_rvalue(v)?))).collect();
    Ok(RValue::List(RList::new(elements?)))
}

/// Try to convert a JSON array of objects into a data.frame.
/// Returns `None` if the array elements are not all objects with the same keys.
fn try_array_as_dataframe(arr: &[serde_json::Value]) -> Result<Option<RValue>, RError> {
    // Collect keys from each object element
    let mut all_objects = true;
    let mut key_sets: Vec<Vec<String>> = Vec::new();

    for item in arr {
        if let serde_json::Value::Object(obj) = item {
            let keys: Vec<String> = obj.keys().cloned().collect();
            key_sets.push(keys);
        } else {
            all_objects = false;
            break;
        }
    }

    if !all_objects || key_sets.is_empty() {
        return Ok(None);
    }

    // Check all objects have the same keys (order-independent)
    let first_keys: std::collections::HashSet<&str> =
        key_sets[0].iter().map(|s| s.as_str()).collect();
    for ks in &key_sets[1..] {
        let this_keys: std::collections::HashSet<&str> = ks.iter().map(|s| s.as_str()).collect();
        if this_keys != first_keys {
            return Ok(None);
        }
    }

    // Build column-oriented data: collect all values for each key
    let col_names: Vec<String> = key_sets[0].clone();
    let nrows = arr.len();
    let mut columns: HashMap<&str, Vec<&serde_json::Value>> = HashMap::new();
    for key in &col_names {
        columns.insert(key.as_str(), Vec::with_capacity(nrows));
    }

    for item in arr {
        if let serde_json::Value::Object(obj) = item {
            for key in &col_names {
                let val = obj.get(key.as_str()).unwrap_or(&serde_json::Value::Null);
                columns.get_mut(key.as_str()).unwrap().push(val);
            }
        }
    }

    // Build each column as an R vector, coercing scalar types
    let mut list_cols: Vec<(Option<String>, RValue)> = Vec::new();
    for key in &col_names {
        let vals = &columns[key.as_str()];
        let col_value = coerce_json_column(vals)?;
        list_cols.push((Some(key.clone()), col_value));
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

/// Coerce a column of JSON values to an R vector.
/// Tries integer -> double -> character, with null becoming NA.
fn coerce_json_column(vals: &[&serde_json::Value]) -> Result<RValue, RError> {
    // Check what types are present
    let mut has_null = false;
    let mut has_bool = false;
    let mut has_int = false;
    let mut has_float = false;
    let mut has_string = false;
    let mut has_complex = false; // arrays/objects

    for v in vals {
        match v {
            serde_json::Value::Null => has_null = true,
            serde_json::Value::Bool(_) => has_bool = true,
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    has_int = true;
                } else {
                    has_float = true;
                }
            }
            serde_json::Value::String(_) => has_string = true,
            _ => has_complex = true,
        }
    }
    let _ = has_null; // null is always compatible (becomes NA)

    // If complex values present, fall back to list column
    if has_complex {
        let elements: Result<Vec<(Option<String>, RValue)>, RError> = vals
            .iter()
            .map(|v| Ok((None, json_to_rvalue(v)?)))
            .collect();
        return Ok(RValue::List(RList::new(elements?)));
    }

    // If strings present, everything becomes character
    if has_string {
        let result: Vec<Option<String>> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::Null => None,
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Bool(b) => Some(if *b { "TRUE" } else { "FALSE" }.to_string()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Character(result.into())));
    }

    // If only booleans (and nulls), produce logical
    if has_bool && !has_int && !has_float {
        let result: Vec<Option<bool>> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::Bool(b) => Some(*b),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Logical(result.into())));
    }

    // If floats present, everything numeric becomes double
    if has_float {
        let result: Vec<Option<f64>> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Double(result.into())));
    }

    // Pure integer (and nulls)
    if has_int {
        let result: Vec<Option<i64>> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::Bool(b) => Some(i64::from(*b)),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(result.into())));
    }

    // If booleans mixed with integers
    if has_bool {
        let result: Vec<Option<i64>> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::Bool(b) => Some(i64::from(*b)),
                serde_json::Value::Number(n) => n.as_i64(),
                _ => None,
            })
            .collect();
        return Ok(RValue::vec(Vector::Integer(result.into())));
    }

    // All null
    let result: Vec<Option<bool>> = vals.iter().map(|_| None).collect();
    Ok(RValue::vec(Vector::Logical(result.into())))
}

/// Try to convert a JSON array of scalars into an atomic vector.
/// Returns `None` if the array contains non-scalar values.
fn try_array_as_vector(arr: &[serde_json::Value]) -> Option<RValue> {
    // Check what types of scalars we have
    let mut has_null = false;
    let mut has_bool = false;
    let mut has_int = false;
    let mut has_float = false;
    let mut has_string = false;

    for item in arr {
        match item {
            serde_json::Value::Null => has_null = true,
            serde_json::Value::Bool(_) => has_bool = true,
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    has_int = true;
                } else {
                    has_float = true;
                }
            }
            serde_json::Value::String(_) => has_string = true,
            // Non-scalar: not a homogeneous scalar array
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => return None,
        }
    }
    let _ = has_null; // null is always compatible

    // Strings dominate
    if has_string {
        let result: Vec<Option<String>> = arr
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Null => None,
                serde_json::Value::Bool(b) => Some(if *b { "TRUE" } else { "FALSE" }.to_string()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Character(result.into())));
    }

    // Pure booleans (no numbers)
    if has_bool && !has_int && !has_float {
        let result: Vec<Option<bool>> = arr
            .iter()
            .map(|v| match v {
                serde_json::Value::Bool(b) => Some(*b),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Logical(result.into())));
    }

    // Has floats -> all numeric becomes double
    if has_float {
        let result: Vec<Option<f64>> = arr
            .iter()
            .map(|v| match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Double(result.into())));
    }

    // Pure integers
    if has_int {
        let result: Vec<Option<i64>> = arr
            .iter()
            .map(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::Bool(b) => Some(i64::from(*b)),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Integer(result.into())));
    }

    // Booleans mixed with numbers -> integer
    if has_bool {
        let result: Vec<Option<i64>> = arr
            .iter()
            .map(|v| match v {
                serde_json::Value::Bool(b) => Some(i64::from(*b)),
                _ => None,
            })
            .collect();
        return Some(RValue::vec(Vector::Integer(result.into())));
    }

    // All null
    let result: Vec<Option<bool>> = arr.iter().map(|_| None).collect();
    Some(RValue::vec(Vector::Logical(result.into())))
}

/// Convert a JSON object to a named list.
fn json_object_to_rvalue(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<RValue, RError> {
    let mut entries: Vec<(Option<String>, RValue)> = Vec::with_capacity(obj.len());
    for (key, value) in obj {
        entries.push((Some(key.clone()), json_to_rvalue(value)?));
    }
    Ok(RValue::List(RList::new(entries)))
}

// endregion

// region: toJSON

/// Convert an R value to a JSON string.
///
/// Conversion rules:
/// - Named list -> JSON object
/// - Unnamed list -> JSON array
/// - Vector of length 1 -> JSON scalar
/// - Vector of length > 1 -> JSON array
/// - NULL -> null
///
/// @param x R value to convert
/// @return character scalar containing the JSON string
#[builtin(name = "toJSON", min_args = 1, names = ["jsonlite::toJSON"], namespace = "jsonlite")]
fn builtin_to_json(args: &[RValue], _named: &[(String, RValue)]) -> Result<RValue, RError> {
    let value = args
        .first()
        .ok_or_else(|| RError::new(RErrorKind::Argument, "argument 'x' is missing".to_string()))?;

    let json = rvalue_to_json(value)?;
    let json_str = serde_json::to_string(&json)
        .map_err(|e| RError::new(RErrorKind::Other, format!("JSON serialization error: {e}")))?;

    Ok(RValue::vec(Vector::Character(vec![Some(json_str)].into())))
}

/// Convert an `RValue` to a `serde_json::Value`.
fn rvalue_to_json(value: &RValue) -> Result<serde_json::Value, RError> {
    match value {
        RValue::Null => Ok(serde_json::Value::Null),
        RValue::Vector(rv) => vector_to_json(&rv.inner),
        RValue::List(list) => list_to_json(list),
        RValue::Function(_) => Err(RError::new(
            RErrorKind::Type,
            "cannot convert function to JSON".to_string(),
        )),
        RValue::Environment(_) => Err(RError::new(
            RErrorKind::Type,
            "cannot convert environment to JSON".to_string(),
        )),
        RValue::Language(_) => Err(RError::new(
            RErrorKind::Type,
            "cannot convert language object to JSON".to_string(),
        )),
    }
}

/// Convert an atomic vector to JSON.
/// Scalars (length 1) become JSON scalars; longer vectors become JSON arrays.
fn vector_to_json(vec: &Vector) -> Result<serde_json::Value, RError> {
    match vec {
        Vector::Logical(v) => {
            if v.len() == 1 {
                match v[0] {
                    Some(b) => Ok(serde_json::Value::Bool(b)),
                    None => Ok(serde_json::Value::Null),
                }
            } else {
                let arr: Vec<serde_json::Value> = v
                    .iter()
                    .map(|x| match x {
                        Some(b) => serde_json::Value::Bool(*b),
                        None => serde_json::Value::Null,
                    })
                    .collect();
                Ok(serde_json::Value::Array(arr))
            }
        }
        Vector::Integer(v) => {
            if v.len() == 1 {
                match v[0] {
                    Some(i) => Ok(serde_json::json!(i)),
                    None => Ok(serde_json::Value::Null),
                }
            } else {
                let arr: Vec<serde_json::Value> = v
                    .iter()
                    .map(|x| match x {
                        Some(i) => serde_json::json!(i),
                        None => serde_json::Value::Null,
                    })
                    .collect();
                Ok(serde_json::Value::Array(arr))
            }
        }
        Vector::Double(v) => {
            if v.len() == 1 {
                match v[0] {
                    Some(f) => double_to_json(f),
                    None => Ok(serde_json::Value::Null),
                }
            } else {
                let arr: Result<Vec<serde_json::Value>, RError> = v
                    .iter()
                    .map(|x| match x {
                        Some(f) => double_to_json(*f),
                        None => Ok(serde_json::Value::Null),
                    })
                    .collect();
                Ok(serde_json::Value::Array(arr?))
            }
        }
        Vector::Character(v) => {
            if v.len() == 1 {
                match &v[0] {
                    Some(s) => Ok(serde_json::Value::String(s.clone())),
                    None => Ok(serde_json::Value::Null),
                }
            } else {
                let arr: Vec<serde_json::Value> = v
                    .iter()
                    .map(|x| match x {
                        Some(s) => serde_json::Value::String(s.clone()),
                        None => serde_json::Value::Null,
                    })
                    .collect();
                Ok(serde_json::Value::Array(arr))
            }
        }
        Vector::Complex(v) => {
            // Represent complex as string "re+imi"
            let arr: Vec<serde_json::Value> = v
                .iter()
                .map(|x| match x {
                    Some(c) => serde_json::Value::String(format!("{}+{}i", c.re, c.im)),
                    None => serde_json::Value::Null,
                })
                .collect();
            if arr.len() == 1 {
                Ok(arr.into_iter().next().unwrap())
            } else {
                Ok(serde_json::Value::Array(arr))
            }
        }
        Vector::Raw(v) => {
            let arr: Vec<serde_json::Value> = v.iter().map(|b| serde_json::json!(*b)).collect();
            if arr.len() == 1 {
                Ok(arr.into_iter().next().unwrap())
            } else {
                Ok(serde_json::Value::Array(arr))
            }
        }
    }
}

/// Convert an f64 to a JSON number, handling special values.
fn double_to_json(f: f64) -> Result<serde_json::Value, RError> {
    if f.is_nan() || f.is_infinite() {
        // JSON has no NaN/Inf, represent as null (matches jsonlite behavior)
        Ok(serde_json::Value::Null)
    } else {
        Ok(serde_json::json!(f))
    }
}

/// Convert an R list to JSON.
/// Named lists become objects; unnamed lists become arrays.
fn list_to_json(list: &RList) -> Result<serde_json::Value, RError> {
    let all_named = !list.values.is_empty() && list.values.iter().all(|(name, _)| name.is_some());

    if all_named {
        let mut map = serde_json::Map::new();
        for (name, value) in &list.values {
            let key = name.as_ref().unwrap().clone();
            map.insert(key, rvalue_to_json(value)?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        let arr: Result<Vec<serde_json::Value>, RError> = list
            .values
            .iter()
            .map(|(_, value)| rvalue_to_json(value))
            .collect();
        Ok(serde_json::Value::Array(arr?))
    }
}

// endregion
