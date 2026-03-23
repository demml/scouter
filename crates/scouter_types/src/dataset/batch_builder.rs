use std::sync::Arc;

use arrow::array::{
    ArrayBuilder, ArrayRef, BooleanBuilder, Date32Builder, Float64Builder, Int64Builder,
    ListBuilder, RecordBatch, StringBuilder, StringDictionaryBuilder, StringViewBuilder,
    TimestampMicrosecondBuilder,
};
use arrow::datatypes::{DataType, Fields, Int16Type, SchemaRef, TimeUnit};
use chrono::{NaiveDate, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::dataset::error::DatasetError;

/// Builds Arrow [`RecordBatch`]es from JSON strings at runtime.
///
/// The schema (with system columns already injected via [`inject_system_columns`]) is
/// provided at construction. Call [`append_json_row`] for each JSON string produced by
/// `pydantic_instance.model_dump_json()`, then [`finish`] to materialise the batch with
/// system columns (`scouter_created_at`, `scouter_partition_date`, `scouter_batch_id`)
/// automatically filled in.
///
/// System columns must be the last three fields in the schema, in the order mandated by
/// [`inject_system_columns`].
pub struct DynamicBatchBuilder {
    /// Full schema including system columns (last 3 fields).
    schema: SchemaRef,
    /// Number of user-defined columns (`schema.fields().len() - 3`).
    user_field_count: usize,
    /// Accumulated JSON values per user field: `columns[i]` is the column for
    /// `schema.field(i)`.  Using `Option<Value>` defers Arrow builder dispatch
    /// until `finish()`, which keeps `append_json_row` allocation-free.
    columns: Vec<Vec<Option<Value>>>,
    /// Number of rows appended so far.
    row_count: usize,
}

impl DynamicBatchBuilder {
    /// Construct a builder for the given schema.
    ///
    /// Panics in debug builds if the schema has fewer than 3 fields (the minimum
    /// needed for the three system columns).
    pub fn new(schema: SchemaRef) -> Self {
        let n_fields = schema.fields().len();
        debug_assert!(
            n_fields >= 3,
            "Schema must contain at least 3 system columns"
        );
        let user_field_count = n_fields.saturating_sub(3);
        Self {
            schema,
            user_field_count,
            columns: vec![Vec::new(); user_field_count],
            row_count: 0,
        }
    }

    /// Parse `json_str` and append one row.
    ///
    /// `json_str` must be a JSON object whose keys cover the user-defined fields.
    /// Missing keys append `null` (valid only for nullable fields — the schema
    /// determines nullability, Arrow validates it at `finish()`).
    ///
    /// Returns an error if `json_str` is not valid JSON or if it is not a JSON object.
    pub fn append_json_row(&mut self, json_str: &str) -> Result<(), DatasetError> {
        let root: Value = serde_json::from_str(json_str)?;
        let obj = root.as_object().ok_or_else(|| {
            DatasetError::SchemaParseError(
                "JSON row must be an object (model_dump_json() output expected)".to_string(),
            )
        })?;

        for (col_idx, field) in self.schema.fields()[..self.user_field_count]
            .iter()
            .enumerate()
        {
            let val = obj.get(field.name()).cloned();
            self.columns[col_idx].push(val);
        }
        self.row_count += 1;
        Ok(())
    }

    /// Number of rows appended so far.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Returns `true` if no rows have been appended.
    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    /// Consume the builder and produce a [`RecordBatch`].
    ///
    /// System columns are automatically injected:
    /// - `scouter_created_at`: current UTC timestamp (microsecond precision)
    /// - `scouter_partition_date`: today's date
    /// - `scouter_batch_id`: a UUID v7 string shared across all rows in this batch
    pub fn finish(self) -> Result<RecordBatch, DatasetError> {
        let n = self.row_count;

        // Build user columns
        let mut arrays: Vec<ArrayRef> = Vec::with_capacity(self.schema.fields().len());
        for (col_idx, field) in self.schema.fields()[..self.user_field_count]
            .iter()
            .enumerate()
        {
            let arr = build_array(&self.columns[col_idx], field.data_type())?;
            arrays.push(arr);
        }

        // --- System columns ---

        // scouter_created_at: Timestamp(Microsecond, UTC)
        let now_us = Utc::now().timestamp_micros();
        let mut ts_builder =
            TimestampMicrosecondBuilder::with_capacity(n).with_timezone("UTC".to_string());
        for _ in 0..n {
            ts_builder.append_value(now_us);
        }
        arrays.push(Arc::new(ts_builder.finish()));

        // scouter_partition_date: Date32 (days since UNIX epoch)
        let today = Utc::now().date_naive();
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch is valid");
        let days_since_epoch = (today - epoch).num_days() as i32;
        let mut date_builder = Date32Builder::with_capacity(n);
        for _ in 0..n {
            date_builder.append_value(days_since_epoch);
        }
        arrays.push(Arc::new(date_builder.finish()));

        // scouter_batch_id: Utf8 — one UUID v7 shared across the entire batch
        let batch_id = Uuid::now_v7().to_string();
        let mut id_builder = StringBuilder::with_capacity(n, n * 36);
        for _ in 0..n {
            id_builder.append_value(&batch_id);
        }
        arrays.push(Arc::new(id_builder.finish()));

        RecordBatch::try_new(self.schema, arrays).map_err(|e| {
            DatasetError::ArrowSchemaError(format!("Failed to create RecordBatch: {e}"))
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a single Arrow [`ArrayRef`] from a column of optional JSON values.
fn build_array(values: &[Option<Value>], data_type: &DataType) -> Result<ArrayRef, DatasetError> {
    match data_type {
        DataType::Int64 => {
            let mut b = Int64Builder::with_capacity(values.len());
            for v in values {
                match v {
                    Some(Value::Number(n)) => match n.as_i64() {
                        Some(i) => b.append_value(i),
                        None => {
                            return Err(DatasetError::SchemaParseError(format!(
                                "Cannot coerce {n} to Int64"
                            )))
                        }
                    },
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected integer, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Float64 => {
            let mut b = Float64Builder::with_capacity(values.len());
            for v in values {
                match v {
                    Some(Value::Number(n)) => match n.as_f64() {
                        Some(f) => b.append_value(f),
                        None => {
                            return Err(DatasetError::SchemaParseError(format!(
                                "Cannot coerce {n} to Float64"
                            )))
                        }
                    },
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected number, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Utf8View => {
            let mut b = StringViewBuilder::with_capacity(values.len());
            for v in values {
                match v {
                    Some(Value::String(s)) => b.append_value(s),
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected string, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        // scouter_batch_id uses plain Utf8, not Utf8View
        DataType::Utf8 => {
            let mut b = StringBuilder::with_capacity(values.len(), values.len() * 8);
            for v in values {
                match v {
                    Some(Value::String(s)) => b.append_value(s),
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected string, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Boolean => {
            let mut b = BooleanBuilder::with_capacity(values.len());
            for v in values {
                match v {
                    Some(Value::Bool(bv)) => b.append_value(*bv),
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected boolean, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let mut b = TimestampMicrosecondBuilder::with_capacity(values.len())
                .with_timezone("UTC".to_string());
            for v in values {
                match v {
                    Some(Value::String(s)) => {
                        let ts = chrono::DateTime::parse_from_rfc3339(s)
                            .map_err(|e| {
                                DatasetError::SchemaParseError(format!(
                                    "Cannot parse '{s}' as RFC3339 datetime: {e}"
                                ))
                            })?
                            .timestamp_micros();
                        b.append_value(ts);
                    }
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected datetime string, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Date32 => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch is valid");
            let mut b = Date32Builder::with_capacity(values.len());
            for v in values {
                match v {
                    Some(Value::String(s)) => {
                        let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                            DatasetError::SchemaParseError(format!(
                                "Cannot parse '{s}' as date (YYYY-MM-DD): {e}"
                            ))
                        })?;
                        let days = (date - epoch).num_days() as i32;
                        b.append_value(days);
                    }
                    Some(Value::Null) | None => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected date string, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(b.finish()))
        }

        DataType::Dictionary(key_type, value_type) => {
            if key_type.as_ref() == &DataType::Int16 && value_type.as_ref() == &DataType::Utf8 {
                let mut b: StringDictionaryBuilder<Int16Type> =
                    StringDictionaryBuilder::with_capacity(values.len(), 16, values.len() * 8);
                for v in values {
                    match v {
                        Some(Value::String(s)) => {
                            b.append_value(s);
                        }
                        Some(Value::Null) | None => b.append_null(),
                        other => {
                            return Err(DatasetError::SchemaParseError(format!(
                                "Expected string for dictionary, got: {other:?}"
                            )))
                        }
                    }
                }
                Ok(Arc::new(b.finish()))
            } else {
                Err(DatasetError::UnsupportedType(format!(
                    "Dictionary({key_type:?}, {value_type:?}) — only Dictionary(Int16, Utf8) is supported"
                )))
            }
        }

        DataType::List(item_field) => {
            let inner_builder = make_builder(item_field.data_type(), values.len())?;
            let mut list_builder = ListBuilder::new(inner_builder);
            for v in values {
                match v {
                    Some(Value::Array(items)) => {
                        let inner = list_builder.values();
                        append_to_builder(inner, items, item_field.data_type())?;
                        list_builder.append(true);
                    }
                    Some(Value::Null) | None => {
                        list_builder.append_null();
                    }
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected array, got: {other:?}"
                        )))
                    }
                }
            }
            Ok(Arc::new(list_builder.finish()))
        }

        DataType::Struct(fields) => build_struct_array(values, fields),

        other => Err(DatasetError::UnsupportedType(format!(
            "Arrow type {other} is not supported by DynamicBatchBuilder"
        ))),
    }
}

/// Build a struct array column from a slice of optional JSON objects.
fn build_struct_array(values: &[Option<Value>], fields: &Fields) -> Result<ArrayRef, DatasetError> {
    // Collect per-subfield columns
    let mut sub_cols: Vec<Vec<Option<Value>>> =
        vec![Vec::with_capacity(values.len()); fields.len()];

    for v in values {
        match v {
            Some(Value::Object(obj)) => {
                for (i, field) in fields.iter().enumerate() {
                    sub_cols[i].push(obj.get(field.name()).cloned());
                }
            }
            Some(Value::Null) | None => {
                for col in sub_cols.iter_mut() {
                    col.push(None);
                }
            }
            other => {
                return Err(DatasetError::SchemaParseError(format!(
                    "Expected JSON object for struct field, got: {other:?}"
                )))
            }
        }
    }

    let sub_arrays: Vec<ArrayRef> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| build_array(&sub_cols[i], field.data_type()))
        .collect::<Result<_, _>>()?;

    // Build null bitmap from the top-level option
    let null_buffer: arrow::buffer::NullBuffer = values
        .iter()
        .map(|v| v.as_ref().map(|v| !v.is_null()).unwrap_or(false))
        .collect();

    let struct_array =
        arrow::array::StructArray::new(fields.clone(), sub_arrays, Some(null_buffer));

    Ok(Arc::new(struct_array))
}

/// Create a concrete [`ArrayBuilder`] for a given Arrow [`DataType`].
/// Used to construct inner builders for [`ListBuilder`].
fn make_builder(
    data_type: &DataType,
    capacity: usize,
) -> Result<Box<dyn ArrayBuilder>, DatasetError> {
    match data_type {
        DataType::Int64 => Ok(Box::new(Int64Builder::with_capacity(capacity))),
        DataType::Float64 => Ok(Box::new(Float64Builder::with_capacity(capacity))),
        DataType::Utf8View => Ok(Box::new(StringViewBuilder::with_capacity(capacity))),
        DataType::Utf8 => Ok(Box::new(StringBuilder::with_capacity(
            capacity,
            capacity * 8,
        ))),
        DataType::Boolean => Ok(Box::new(BooleanBuilder::with_capacity(capacity))),
        DataType::Timestamp(TimeUnit::Microsecond, _) => Ok(Box::new(
            TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC".to_string()),
        )),
        DataType::Date32 => Ok(Box::new(Date32Builder::with_capacity(capacity))),
        other => Err(DatasetError::UnsupportedType(format!(
            "Cannot create list item builder for {other}"
        ))),
    }
}

/// Append a slice of JSON values to an existing `dyn ArrayBuilder`.
/// Used when filling the inner builder of a [`ListBuilder`].
fn append_to_builder(
    builder: &mut dyn ArrayBuilder,
    items: &[Value],
    data_type: &DataType,
) -> Result<(), DatasetError> {
    match data_type {
        DataType::Int64 => {
            let b = builder
                .as_any_mut()
                .downcast_mut::<Int64Builder>()
                .ok_or_else(|| {
                    DatasetError::SchemaParseError(
                        "Internal error: builder type mismatch for Int64".to_string(),
                    )
                })?;
            for v in items {
                match v {
                    Value::Number(n) => b.append_value(n.as_i64().ok_or_else(|| {
                        DatasetError::SchemaParseError(format!("Cannot coerce {n} to Int64"))
                    })?),
                    Value::Null => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected integer in list, got: {other:?}"
                        )))
                    }
                }
            }
        }
        DataType::Float64 => {
            let b = builder
                .as_any_mut()
                .downcast_mut::<Float64Builder>()
                .ok_or_else(|| {
                    DatasetError::SchemaParseError(
                        "Internal error: builder type mismatch for Float64".to_string(),
                    )
                })?;
            for v in items {
                match v {
                    Value::Number(n) => b.append_value(n.as_f64().ok_or_else(|| {
                        DatasetError::SchemaParseError(format!("Cannot coerce {n} to Float64"))
                    })?),
                    Value::Null => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected number in list, got: {other:?}"
                        )))
                    }
                }
            }
        }
        DataType::Utf8View => {
            let b = builder
                .as_any_mut()
                .downcast_mut::<StringViewBuilder>()
                .ok_or_else(|| {
                    DatasetError::SchemaParseError(
                        "Internal error: builder type mismatch for Utf8View".to_string(),
                    )
                })?;
            for v in items {
                match v {
                    Value::String(s) => b.append_value(s),
                    Value::Null => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected string in list, got: {other:?}"
                        )))
                    }
                }
            }
        }
        DataType::Boolean => {
            let b = builder
                .as_any_mut()
                .downcast_mut::<BooleanBuilder>()
                .ok_or_else(|| {
                    DatasetError::SchemaParseError(
                        "Internal error: builder type mismatch for Boolean".to_string(),
                    )
                })?;
            for v in items {
                match v {
                    Value::Bool(bv) => b.append_value(*bv),
                    Value::Null => b.append_null(),
                    other => {
                        return Err(DatasetError::SchemaParseError(format!(
                            "Expected boolean in list, got: {other:?}"
                        )))
                    }
                }
            }
        }
        other => {
            return Err(DatasetError::UnsupportedType(format!(
                "List item type {other} is not supported"
            )))
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::schema::{
        inject_system_columns, json_schema_to_arrow, SCOUTER_BATCH_ID, SCOUTER_CREATED_AT,
        SCOUTER_PARTITION_DATE,
    };
    use arrow::array::{
        Array, BooleanArray, Date32Array, Float64Array, Int64Array, TimestampMicrosecondArray,
    };
    use arrow::datatypes::DataType;

    fn schema_from_json(json: &str) -> SchemaRef {
        let schema = json_schema_to_arrow(json).unwrap();
        Arc::new(inject_system_columns(schema).unwrap())
    }

    fn flat_schema() -> SchemaRef {
        schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "user_id": {"type": "string"},
                    "value": {"type": "number"},
                    "count": {"type": "integer"},
                    "active": {"type": "boolean"}
                },
                "required": ["user_id", "value", "count", "active"]
            }"#,
        )
    }

    #[test]
    fn test_flat_types_round_trip() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema.clone());
        b.append_json_row(r#"{"user_id":"alice","value":1.5,"count":3,"active":true}"#)
            .unwrap();
        b.append_json_row(r#"{"user_id":"bob","value":2.0,"count":7,"active":false}"#)
            .unwrap();
        assert_eq!(b.row_count(), 2);

        let batch = b.finish().unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.schema(), schema);

        // spot-check user columns
        let val_col = batch
            .column_by_name("value")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert!((val_col.value(0) - 1.5).abs() < f64::EPSILON);

        let cnt_col = batch
            .column_by_name("count")
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        assert_eq!(cnt_col.value(1), 7);

        let active_col = batch
            .column_by_name("active")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap();
        assert!(!active_col.value(1));
    }

    #[test]
    fn test_system_columns_injected() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"user_id":"x","value":0.0,"count":0,"active":false}"#)
            .unwrap();
        let batch = b.finish().unwrap();

        // scouter_created_at
        let ts = batch
            .column_by_name(SCOUTER_CREATED_AT)
            .unwrap()
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap();
        assert!(ts.value(0) > 0);

        // scouter_partition_date
        let date = batch
            .column_by_name(SCOUTER_PARTITION_DATE)
            .unwrap()
            .as_any()
            .downcast_ref::<Date32Array>()
            .unwrap();
        // days since epoch should be positive (we're past 1970)
        assert!(date.value(0) > 0);

        // scouter_batch_id is shared across all rows
        let ids = batch.column_by_name(SCOUTER_BATCH_ID).unwrap();
        assert_eq!(ids.len(), 1);
        assert!(!ids.is_null(0));
    }

    #[test]
    fn test_batch_id_shared_across_rows() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        for _ in 0..5 {
            b.append_json_row(r#"{"user_id":"u","value":0.0,"count":0,"active":true}"#)
                .unwrap();
        }
        let batch = b.finish().unwrap();
        let ids: Vec<String> = (0..5)
            .map(|i| {
                arrow::array::as_string_array(batch.column_by_name(SCOUTER_BATCH_ID).unwrap())
                    .value(i)
                    .to_string()
            })
            .collect();
        // All rows in a batch share the same UUID
        assert!(ids.windows(2).all(|w| w[0] == w[1]));
        // UUID is non-empty
        assert_eq!(ids[0].len(), 36);
    }

    #[test]
    fn test_nullable_fields() {
        let schema = schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "age": {"anyOf": [{"type": "integer"}, {"type": "null"}]}
                },
                "required": ["name"]
            }"#,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"name":"alice","age":30}"#).unwrap();
        b.append_json_row(r#"{"name":"bob","age":null}"#).unwrap();
        b.append_json_row(r#"{"name":"carol"}"#).unwrap(); // missing → null

        let batch = b.finish().unwrap();
        let age = batch
            .column_by_name("age")
            .unwrap()
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        assert_eq!(age.value(0), 30);
        assert!(age.is_null(1));
        assert!(age.is_null(2));
    }

    #[test]
    fn test_timestamp_parsing() {
        let schema = schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "ts": {"type": "string", "format": "date-time"}
                },
                "required": ["ts"]
            }"#,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"ts":"2024-06-01T12:00:00Z"}"#)
            .unwrap();
        let batch = b.finish().unwrap();
        let ts = batch
            .column_by_name("ts")
            .unwrap()
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap();
        // 2024-06-01T12:00:00Z = 1717243200000000 µs
        assert_eq!(ts.value(0), 1_717_243_200_000_000);
    }

    #[test]
    fn test_date_parsing() {
        let schema = schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "d": {"type": "string", "format": "date"}
                },
                "required": ["d"]
            }"#,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"d":"1970-01-02"}"#).unwrap();
        let batch = b.finish().unwrap();
        let dates = batch
            .column_by_name("d")
            .unwrap()
            .as_any()
            .downcast_ref::<Date32Array>()
            .unwrap();
        assert_eq!(dates.value(0), 1); // 1 day after epoch
    }

    #[test]
    fn test_nested_struct() {
        let schema = schema_from_json(
            r##"{
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "addr": {"$ref": "#/$defs/Addr"}
                },
                "required": ["id", "addr"],
                "$defs": {
                    "Addr": {
                        "type": "object",
                        "properties": {
                            "city": {"type": "string"},
                            "zip": {"type": "string"}
                        },
                        "required": ["city", "zip"]
                    }
                }
            }"##,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"id":"1","addr":{"city":"NYC","zip":"10001"}}"#)
            .unwrap();
        let batch = b.finish().unwrap();
        let addr_col = batch.column_by_name("addr").unwrap();
        assert!(matches!(addr_col.data_type(), DataType::Struct(_)));
        assert!(!addr_col.is_null(0));
    }

    #[test]
    fn test_list_field() {
        let schema = schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "scores": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["scores"]
            }"#,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"scores":[1.0,2.5,3.0]}"#).unwrap();
        let batch = b.finish().unwrap();
        let scores = batch.column_by_name("scores").unwrap();
        assert!(matches!(scores.data_type(), DataType::List(_)));
        assert_eq!(scores.len(), 1);
    }

    #[test]
    fn test_dictionary_field() {
        let schema = schema_from_json(
            r#"{
                "type": "object",
                "properties": {
                    "status": {"enum": ["active","inactive"]}
                },
                "required": ["status"]
            }"#,
        );
        let mut b = DynamicBatchBuilder::new(schema);
        b.append_json_row(r#"{"status":"active"}"#).unwrap();
        b.append_json_row(r#"{"status":"inactive"}"#).unwrap();
        let batch = b.finish().unwrap();
        let status = batch.column_by_name("status").unwrap();
        assert!(matches!(status.data_type(), DataType::Dictionary(_, _)));
    }

    #[test]
    fn test_empty_builder_finish() {
        let schema = flat_schema();
        let b = DynamicBatchBuilder::new(schema.clone());
        assert!(b.is_empty());
        let batch = b.finish().unwrap();
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.schema(), schema);
    }

    #[test]
    fn test_malformed_json_error() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        let err = b.append_json_row("{not valid json}").unwrap_err();
        assert!(matches!(err, DatasetError::SerializationError(_)));
    }

    #[test]
    fn test_non_object_json_error() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        let err = b
            .append_json_row(r#"["array","not","object"]"#)
            .unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
    }

    #[test]
    fn test_type_mismatch_int_error() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        // "count" is Int64, but we pass a string
        b.append_json_row(r#"{"user_id":"u","value":1.0,"count":"bad","active":true}"#)
            .unwrap(); // append succeeds (we defer type checking to finish)

        // build_array is called at finish, so error surfaces there
        let err = b.finish().unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
    }

    #[test]
    fn test_row_count_matches() {
        let schema = flat_schema();
        let mut b = DynamicBatchBuilder::new(schema);
        for i in 0..42 {
            b.append_json_row(&format!(
                r#"{{"user_id":"u{i}","value":{i}.0,"count":{i},"active":true}}"#
            ))
            .unwrap();
        }
        assert_eq!(b.row_count(), 42);
        let batch = b.finish().unwrap();
        assert_eq!(batch.num_rows(), 42);
    }
}
