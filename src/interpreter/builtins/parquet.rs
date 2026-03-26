//! Parquet I/O builtins — `read.parquet()` and `write.parquet()` for reading
//! and writing Apache Parquet files as data frames.
//!
//! Feature-gated behind the `parquet` feature flag. Uses the `parquet` and
//! `arrow` crates to convert between Arrow columnar format and R values.

use std::fs::File;
use std::sync::Arc;

use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, Int16Array, Int32Array, Int64Array, Int8Array,
    RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array, UInt8Array,
};
use arrow::datatypes::{DataType as ArrowType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;

use super::dataframes::build_data_frame;
use super::CallArgs;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

// region: helpers

/// Convert an Arrow array column to an R vector.
fn arrow_column_to_rvector(col: &dyn Array) -> Result<RValue, RError> {
    match col.data_type() {
        ArrowType::Boolean => {
            let arr = col.as_any().downcast_ref::<BooleanArray>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow BooleanArray".to_string(),
                )
            })?;
            let vals: Vec<Option<bool>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Logical(vals.into())))
        }

        ArrowType::Int8 => {
            let arr = col.as_any().downcast_ref::<Int8Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Int8Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::Int16 => {
            let arr = col.as_any().downcast_ref::<Int16Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Int16Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::Int32 => {
            let arr = col.as_any().downcast_ref::<Int32Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Int32Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::Int64 => {
            // R has no native 64-bit integer; use Integer (our Integer is i64)
            let arr = col.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Int64Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::UInt8 => {
            let arr = col.as_any().downcast_ref::<UInt8Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow UInt8Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::UInt16 => {
            let arr = col.as_any().downcast_ref::<UInt16Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow UInt16Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::UInt32 => {
            let arr = col.as_any().downcast_ref::<UInt32Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow UInt32Array".to_string(),
                )
            })?;
            let vals: Vec<Option<i64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(i64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Integer(vals.into())))
        }

        ArrowType::UInt64 => {
            // UInt64 may overflow i64; store as Double
            let arr = col.as_any().downcast_ref::<UInt64Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow UInt64Array".to_string(),
                )
            })?;
            let vals: Vec<Option<f64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i) as f64)
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(vals.into())))
        }

        ArrowType::Float32 => {
            let arr = col.as_any().downcast_ref::<Float32Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Float32Array".to_string(),
                )
            })?;
            let vals: Vec<Option<f64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(f64::from(arr.value(i)))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(vals.into())))
        }

        ArrowType::Float64 => {
            let arr = col.as_any().downcast_ref::<Float64Array>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow Float64Array".to_string(),
                )
            })?;
            let vals: Vec<Option<f64>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i))
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Double(vals.into())))
        }

        ArrowType::Utf8 => {
            let arr = col.as_any().downcast_ref::<StringArray>().ok_or_else(|| {
                RError::new(
                    RErrorKind::Type,
                    "failed to downcast Arrow StringArray".to_string(),
                )
            })?;
            let vals: Vec<Option<String>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i).to_string())
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Character(vals.into())))
        }

        ArrowType::LargeUtf8 => {
            // LargeUtf8 uses i64 offsets; same string values
            let arr = col
                .as_any()
                .downcast_ref::<arrow::array::LargeStringArray>()
                .ok_or_else(|| {
                    RError::new(
                        RErrorKind::Type,
                        "failed to downcast Arrow LargeStringArray".to_string(),
                    )
                })?;
            let vals: Vec<Option<String>> = (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        None
                    } else {
                        Some(arr.value(i).to_string())
                    }
                })
                .collect();
            Ok(RValue::vec(Vector::Character(vals.into())))
        }

        other => Err(RError::new(
            RErrorKind::Type,
            format!(
                "unsupported Arrow data type '{}' in Parquet file — \
                 only boolean, integer, float, and string columns are supported",
                other
            ),
        )),
    }
}

/// Convert an R vector to an Arrow ArrayRef for writing.
fn rvector_to_arrow_array(vec: &Vector, len: usize) -> Result<Arc<dyn Array>, RError> {
    match vec {
        Vector::Logical(v) => {
            let arr = BooleanArray::from(
                (0..len)
                    .map(|i| v.get(i).copied().flatten())
                    .collect::<Vec<Option<bool>>>(),
            );
            Ok(Arc::new(arr))
        }
        Vector::Integer(v) => {
            let arr =
                Int64Array::from((0..len).map(|i| v.get_opt(i)).collect::<Vec<Option<i64>>>());
            Ok(Arc::new(arr))
        }
        Vector::Double(v) => {
            let arr =
                Float64Array::from((0..len).map(|i| v.get_opt(i)).collect::<Vec<Option<f64>>>());
            Ok(Arc::new(arr))
        }
        Vector::Character(v) => {
            let arr = StringArray::from(
                (0..len)
                    .map(|i| v.get(i).and_then(|s| s.as_deref()))
                    .collect::<Vec<Option<&str>>>(),
            );
            Ok(Arc::new(arr))
        }
        other => Err(RError::new(
            RErrorKind::Type,
            format!(
                "cannot write {} vector to Parquet — \
                 only logical, integer, double, and character vectors are supported",
                other.type_name()
            ),
        )),
    }
}

/// Map an R vector type to an Arrow DataType for schema construction.
fn rvector_to_arrow_type(vec: &Vector) -> Result<ArrowType, RError> {
    match vec {
        Vector::Logical(_) => Ok(ArrowType::Boolean),
        Vector::Integer(_) => Ok(ArrowType::Int64),
        Vector::Double(_) => Ok(ArrowType::Float64),
        Vector::Character(_) => Ok(ArrowType::Utf8),
        other => Err(RError::new(
            RErrorKind::Type,
            format!(
                "cannot determine Parquet type for {} vector — \
                 only logical, integer, double, and character vectors are supported",
                other.type_name()
            ),
        )),
    }
}

// endregion

// region: read.parquet

/// Read a Parquet file into a data.frame.
///
/// Converts Arrow column types to R vectors:
/// - Int8/Int16/Int32/Int64 -> Integer
/// - UInt8/UInt16/UInt32 -> Integer
/// - UInt64/Float32/Float64 -> Double
/// - Boolean -> Logical
/// - Utf8/LargeUtf8 -> Character
/// - null values -> NA
///
/// @param file character scalar: path to the Parquet file
/// @param columns character vector: optional column names to read (default: all)
/// @return data.frame with columns from the Parquet file
#[interpreter_builtin(name = "read.parquet", min_args = 1, namespace = "arrow")]
fn builtin_read_parquet(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let path = call_args.string("file", 0)?;
    let resolved = context.interpreter().resolve_path(&path);

    let file = File::open(&resolved).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot open Parquet file '{}': {}", resolved.display(), e),
        )
    })?;

    let mut builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to read Parquet metadata from '{}': {}", path, e),
        )
    })?;

    // Handle optional column selection
    let selected_columns: Option<Vec<String>> =
        call_args.value("columns", 1).and_then(|v| match v {
            RValue::Null => None,
            RValue::Vector(rv) => {
                let chars = rv.inner.to_characters();
                let names: Vec<String> = chars.into_iter().flatten().collect();
                if names.is_empty() {
                    None
                } else {
                    Some(names)
                }
            }
            _ => None,
        });

    if let Some(ref cols) = selected_columns {
        let parquet_schema = builder.parquet_schema();
        let arrow_schema = builder.schema();

        // Map column names to root column indices
        let mut indices = Vec::new();
        let mut errors = Vec::new();
        for col_name in cols {
            match arrow_schema
                .fields()
                .iter()
                .position(|f| f.name() == col_name)
            {
                Some(idx) => indices.push(idx),
                None => errors.push(format!("column '{}' not found in Parquet file", col_name)),
            }
        }
        if !errors.is_empty() {
            return Err(RError::new(RErrorKind::Other, errors.join("; ")));
        }

        let mask = parquet::arrow::ProjectionMask::roots(parquet_schema, indices.iter().copied());
        builder = builder.with_projection(mask);
    }

    let reader = builder.build().map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to build Parquet reader for '{}': {}", path, e),
        )
    })?;

    // Collect all batches
    let mut all_batches: Vec<RecordBatch> = Vec::new();
    for batch_result in reader {
        let batch = batch_result.map_err(|e| {
            RError::new(
                RErrorKind::Other,
                format!("error reading Parquet batch from '{}': {}", path, e),
            )
        })?;
        all_batches.push(batch);
    }

    if all_batches.is_empty() {
        // Return empty data frame
        return build_data_frame(vec![], 0);
    }

    // Get column names from the schema of the first batch
    let schema = all_batches[0].schema();
    let col_names: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
    let ncols = col_names.len();

    // For each column, concatenate values across all batches
    let mut columns: Vec<(Option<String>, RValue)> = Vec::with_capacity(ncols);
    let mut total_rows: usize = 0;

    for (col_idx, col_name) in col_names.into_iter().enumerate() {
        // Collect all arrays for this column across batches
        let mut col_values: Vec<RValue> = Vec::new();
        for batch in &all_batches {
            let arrow_col = batch.column(col_idx);
            col_values.push(arrow_column_to_rvector(arrow_col.as_ref())?);
        }

        // Concatenate the column values
        let combined = if col_values.len() == 1 {
            col_values.into_iter().next().ok_or_else(|| {
                RError::new(
                    RErrorKind::Other,
                    "empty column in parquet read".to_string(),
                )
            })?
        } else {
            concatenate_rvectors(col_values)?
        };

        if col_idx == 0 {
            total_rows = combined.length();
        }
        columns.push((Some(col_name), combined));
    }

    build_data_frame(columns, total_rows)
}

/// Concatenate multiple R vectors of the same type into one.
fn concatenate_rvectors(values: Vec<RValue>) -> Result<RValue, RError> {
    if values.is_empty() {
        return Ok(RValue::Null);
    }

    // Determine type from the first value
    match &values[0] {
        RValue::Vector(rv) => match &rv.inner {
            Vector::Logical(_) => {
                let mut result: Vec<Option<bool>> = Vec::new();
                for v in &values {
                    if let RValue::Vector(rv) = v {
                        if let Vector::Logical(l) = &rv.inner {
                            result.extend_from_slice(l);
                        }
                    }
                }
                Ok(RValue::vec(Vector::Logical(result.into())))
            }
            Vector::Integer(_) => {
                let mut result: Vec<Option<i64>> = Vec::new();
                for v in &values {
                    if let RValue::Vector(rv) = v {
                        if let Vector::Integer(l) = &rv.inner {
                            result.extend(l.iter_opt());
                        }
                    }
                }
                Ok(RValue::vec(Vector::Integer(result.into())))
            }
            Vector::Double(_) => {
                let mut result: Vec<Option<f64>> = Vec::new();
                for v in &values {
                    if let RValue::Vector(rv) = v {
                        if let Vector::Double(l) = &rv.inner {
                            result.extend(l.iter_opt());
                        }
                    }
                }
                Ok(RValue::vec(Vector::Double(result.into())))
            }
            Vector::Character(_) => {
                let mut result: Vec<Option<String>> = Vec::new();
                for v in &values {
                    if let RValue::Vector(rv) = v {
                        if let Vector::Character(l) = &rv.inner {
                            result.extend_from_slice(l);
                        }
                    }
                }
                Ok(RValue::vec(Vector::Character(result.into())))
            }
            _ => Err(RError::new(
                RErrorKind::Type,
                "unexpected vector type during Parquet column concatenation".to_string(),
            )),
        },
        _ => Err(RError::new(
            RErrorKind::Type,
            "unexpected value type during Parquet column concatenation".to_string(),
        )),
    }
}

// endregion

// region: write.parquet

/// Write a data.frame to a Parquet file.
///
/// Converts R vectors to Arrow column types:
/// - Logical -> Boolean
/// - Integer -> Int64
/// - Double -> Float64
/// - Character -> Utf8
///
/// @param x data.frame to write
/// @param file character scalar: output file path
/// @return NULL (invisibly)
#[interpreter_builtin(name = "write.parquet", min_args = 2, namespace = "arrow")]
fn builtin_write_parquet(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);

    let df = call_args.value("x", 0).ok_or_else(|| {
        RError::new(
            RErrorKind::Argument,
            "argument 'x' is missing, with no default".to_string(),
        )
    })?;

    let list: &RList = match df {
        RValue::List(l) => l,
        _ => {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "write.parquet() requires a data.frame, got {}",
                    df.type_name()
                ),
            ))
        }
    };

    let path = call_args.string("file", 1)?;
    let resolved = context.interpreter().resolve_path(&path);

    // Extract column names
    let col_names: Vec<String> = match list.get_attr("names") {
        Some(RValue::Vector(rv)) => rv
            .inner
            .to_characters()
            .into_iter()
            .enumerate()
            .map(|(i, s)| s.unwrap_or_else(|| format!("V{}", i + 1)))
            .collect(),
        _ => list
            .values
            .iter()
            .enumerate()
            .map(|(i, (name, _))| name.clone().unwrap_or_else(|| format!("V{}", i + 1)))
            .collect(),
    };

    // Build Arrow schema and arrays
    let mut fields: Vec<Field> = Vec::new();
    let mut arrays: Vec<Arc<dyn Array>> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (i, (_, val)) in list.values.iter().enumerate() {
        let col_name = col_names
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("V{}", i + 1));

        match val {
            RValue::Vector(rv) => {
                let len = rv.inner.len();
                match rvector_to_arrow_type(&rv.inner) {
                    Ok(arrow_type) => {
                        fields.push(Field::new(&col_name, arrow_type, true));
                        match rvector_to_arrow_array(&rv.inner, len) {
                            Ok(arr) => arrays.push(arr),
                            Err(e) => errors.push(format!("column '{}': {}", col_name, e)),
                        }
                    }
                    Err(e) => errors.push(format!("column '{}': {}", col_name, e)),
                }
            }
            _ => {
                errors.push(format!(
                    "column '{}': expected vector, got {}",
                    col_name,
                    val.type_name()
                ));
            }
        }
    }

    if !errors.is_empty() {
        return Err(RError::new(RErrorKind::Other, errors.join("; ")));
    }

    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to create Arrow RecordBatch: {}", e),
        )
    })?;

    let file = File::create(&resolved).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot create Parquet file '{}': {}", resolved.display(), e),
        )
    })?;

    let mut writer = ArrowWriter::try_new(file, schema, None).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to create Parquet writer: {}", e),
        )
    })?;

    writer.write(&batch).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to write Parquet data: {}", e),
        )
    })?;

    writer.close().map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to finalize Parquet file: {}", e),
        )
    })?;

    Ok(RValue::Null)
}

// endregion
