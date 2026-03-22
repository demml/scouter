use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Fields, Schema, TimeUnit};
use serde_json::{Map, Value};

use crate::dataset::error::DatasetError;
use crate::dataset::types::{DatasetFingerprint, DatasetNamespace};

pub const SCOUTER_CREATED_AT: &str = "scouter_created_at";
pub const SCOUTER_PARTITION_DATE: &str = "scouter_partition_date";
pub const SCOUTER_BATCH_ID: &str = "scouter_batch_id";

const MAX_SCHEMA_DEPTH: usize = 32;

/// Convert a Pydantic-generated JSON Schema string into an Arrow `Schema`.
///
/// Handles:
/// - Scalar types: integer, number, string (with date/date-time format), boolean
/// - Optional[T] via `anyOf: [{T}, {type: "null"}]`
/// - Nested models via `$ref` → `$defs` resolution
/// - List[T] via `type: "array"`
/// - Enum/Literal via `enum` key → Dictionary(Int16, Utf8)
///
/// System columns (`scouter_created_at`, `scouter_partition_date`, `scouter_batch_id`)
/// are NOT injected here — call `inject_system_columns()` after.
pub fn json_schema_to_arrow(json_schema: &str) -> Result<Schema, DatasetError> {
    let root: Value = serde_json::from_str(json_schema)?;

    let obj = root.as_object().ok_or_else(|| {
        DatasetError::SchemaParseError("JSON Schema root must be an object".to_string())
    })?;

    let defs = obj
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let properties = obj
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DatasetError::SchemaParseError(
                "JSON Schema must have a 'properties' key at the root".to_string(),
            )
        })?;

    let required: std::collections::HashSet<&str> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut fields = Vec::with_capacity(properties.len());
    for (name, prop) in properties {
        let nullable = !required.contains(name.as_str());
        let (dtype, is_nullable) = resolve_type(prop, &defs, nullable, 0)?;
        fields.push(Field::new(name, dtype, is_nullable));
    }

    Ok(Schema::new(fields))
}

/// Inject the three system columns at the end of a schema.
/// These are always non-nullable and always appended in the same order.
///
/// Returns `Err` if the user schema already contains a reserved column name.
pub fn inject_system_columns(schema: Schema) -> Result<Schema, DatasetError> {
    for col_name in [SCOUTER_CREATED_AT, SCOUTER_PARTITION_DATE, SCOUTER_BATCH_ID] {
        if schema.index_of(col_name).is_ok() {
            return Err(DatasetError::SchemaParseError(format!(
                "User schema must not contain reserved column '{col_name}'"
            )));
        }
    }
    let mut fields: Vec<Field> = schema.fields().iter().map(|f| f.as_ref().clone()).collect();
    fields.push(Field::new(
        SCOUTER_CREATED_AT,
        DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC"))),
        false,
    ));
    fields.push(Field::new(SCOUTER_PARTITION_DATE, DataType::Date32, false));
    fields.push(Field::new(SCOUTER_BATCH_ID, DataType::Utf8, false));
    Ok(Schema::new(fields))
}

/// Compute a stable fingerprint from a schema.
///
/// Serializes the schema to a canonical string (sorted field names + type + nullable),
/// then hashes with SHA-256.
pub fn schema_fingerprint(schema: &Schema) -> Result<DatasetFingerprint, DatasetError> {
    let canonical = canonical_schema_repr(schema);
    Ok(DatasetFingerprint::from_schema_json(&canonical))
}

fn canonical_type_repr(dt: &DataType) -> String {
    match dt {
        DataType::Struct(fields) => {
            let mut sub: Vec<String> = fields
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}:{}",
                        f.name(),
                        canonical_type_repr(f.data_type()),
                        f.is_nullable()
                    )
                })
                .collect();
            sub.sort();
            format!("Struct({})", sub.join(","))
        }
        other => format!("{other}"),
    }
}

fn canonical_schema_repr(schema: &Schema) -> String {
    let mut fields: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| {
            format!(
                "{}:{}:{}",
                f.name(),
                canonical_type_repr(f.data_type()),
                f.is_nullable()
            )
        })
        .collect();
    fields.sort();
    fields.join("|")
}

/// Returns true if this JSON Schema variant represents the `null` type,
/// covering Pydantic v2's multiple encodings:
/// - `{"type": "null"}`
/// - `{"const": null}`
/// - `{"enum": [null]}` (single-element null enum)
fn is_null_variant(v: &Value) -> bool {
    if v.get("type").and_then(Value::as_str) == Some("null") {
        return true;
    }
    if v.get("const").map(Value::is_null).unwrap_or(false) {
        return true;
    }
    if let Some(arr) = v.get("enum").and_then(Value::as_array) {
        if arr.len() == 1 && arr[0].is_null() {
            return true;
        }
    }
    false
}

/// Resolve a single JSON Schema property into an Arrow `(DataType, nullable)` pair.
fn resolve_type(
    prop: &Value,
    defs: &Map<String, Value>,
    nullable: bool,
    depth: usize,
) -> Result<(DataType, bool), DatasetError> {
    if depth >= MAX_SCHEMA_DEPTH {
        return Err(DatasetError::SchemaParseError(format!(
            "Schema nesting exceeds maximum depth of {MAX_SCHEMA_DEPTH}"
        )));
    }

    let obj = match prop.as_object() {
        Some(o) => o,
        None => {
            return Err(DatasetError::SchemaParseError(
                "Property must be a JSON object".to_string(),
            ))
        }
    };

    // $ref — look up in $defs
    if let Some(ref_val) = obj.get("$ref").and_then(Value::as_str) {
        return resolve_ref(ref_val, defs, nullable, depth + 1);
    }

    // anyOf — typically Optional[T]: [{T}, {type: "null"}]
    if let Some(any_of) = obj.get("anyOf").and_then(Value::as_array) {
        return resolve_any_of(any_of, defs, depth + 1);
    }

    // enum / Literal
    if obj.contains_key("enum") {
        return Ok((
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            nullable,
        ));
    }

    let type_str = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| DatasetError::UnsupportedType(format!("No 'type' in: {prop}")))?;

    match type_str {
        "integer" => Ok((DataType::Int64, nullable)),
        "number" => Ok((DataType::Float64, nullable)),
        "boolean" => Ok((DataType::Boolean, nullable)),
        "string" => {
            let format = obj.get("format").and_then(Value::as_str);
            match format {
                Some("date-time") => Ok((
                    DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::from("UTC"))),
                    nullable,
                )),
                Some("date") => Ok((DataType::Date32, nullable)),
                _ => Ok((DataType::Utf8View, nullable)),
            }
        }
        "array" => {
            let items = obj.get("items").ok_or_else(|| {
                DatasetError::SchemaParseError("Array missing 'items'".to_string())
            })?;
            let (item_type, item_nullable) = resolve_type(items, defs, true, depth + 1)?;
            let item_field = Arc::new(Field::new("item", item_type, item_nullable));
            Ok((DataType::List(item_field), nullable))
        }
        "object" => {
            let props = obj
                .get("properties")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    DatasetError::UnsupportedType(
                        "Free-form dict (object without 'properties') is not yet supported"
                            .to_string(),
                    )
                })?;
            let required: std::collections::HashSet<&str> = obj
                .get("required")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let mut struct_fields = Vec::with_capacity(props.len());
            for (name, sub_prop) in props {
                let field_nullable = !required.contains(name.as_str());
                let (dtype, is_nullable) = resolve_type(sub_prop, defs, field_nullable, depth + 1)?;
                struct_fields.push(Arc::new(Field::new(name, dtype, is_nullable)));
            }
            Ok((DataType::Struct(Fields::from(struct_fields)), nullable))
        }
        "null" => Ok((DataType::Null, true)),
        other => Err(DatasetError::UnsupportedType(other.to_string())),
    }
}

/// Resolve `{"$ref": "#/$defs/SomeName"}` to an Arrow DataType.
///
/// Handles two cases:
/// - Object defs with `properties` → Struct
/// - Non-object defs (e.g., enum, primitive) → delegated back to `resolve_type`
fn resolve_ref(
    ref_val: &str,
    defs: &Map<String, Value>,
    nullable: bool,
    depth: usize,
) -> Result<(DataType, bool), DatasetError> {
    if depth >= MAX_SCHEMA_DEPTH {
        return Err(DatasetError::SchemaParseError(format!(
            "Schema nesting exceeds maximum depth of {MAX_SCHEMA_DEPTH}"
        )));
    }

    let def_name = ref_val.strip_prefix("#/$defs/").ok_or_else(|| {
        DatasetError::RefResolutionError(format!("Unrecognized $ref format: {ref_val}"))
    })?;

    let def = defs.get(def_name).ok_or_else(|| {
        DatasetError::RefResolutionError(format!("$defs entry not found: {def_name}"))
    })?;

    let def_obj = def.as_object().ok_or_else(|| {
        DatasetError::RefResolutionError(format!("$defs entry '{def_name}' is not an object"))
    })?;

    // Struct def (nested Pydantic model)
    if let Some(props) = def_obj.get("properties").and_then(Value::as_object) {
        let required: std::collections::HashSet<&str> = def_obj
            .get("required")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let mut struct_fields = Vec::with_capacity(props.len());
        for (name, sub_prop) in props {
            let field_nullable = !required.contains(name.as_str());
            let (dtype, is_nullable) = resolve_type(sub_prop, defs, field_nullable, depth + 1)?;
            struct_fields.push(Arc::new(Field::new(name, dtype, is_nullable)));
        }
        return Ok((DataType::Struct(Fields::from(struct_fields)), nullable));
    }

    // Non-struct def (enum, primitive, etc.) — delegate to resolve_type
    resolve_type(def, defs, nullable, depth + 1)
}

/// Handle `anyOf` — Pydantic's encoding for `Optional[T]` is `[{T}, {"type": "null"}]`.
/// We find the non-null variant and mark it nullable.
fn resolve_any_of(
    variants: &[Value],
    defs: &Map<String, Value>,
    depth: usize,
) -> Result<(DataType, bool), DatasetError> {
    let non_null: Vec<&Value> = variants.iter().filter(|v| !is_null_variant(v)).collect();

    if non_null.len() == 1 {
        let (dtype, _) = resolve_type(non_null[0], defs, true, depth)?;
        return Ok((dtype, true));
    }

    // Multiple non-null variants — not yet supported
    Err(DatasetError::UnsupportedType(
        "anyOf with multiple non-null variants is not supported".to_string(),
    ))
}

/// Compute an Arrow schema fingerprint from a Pydantic JSON Schema string.
/// Convenience wrapper: parse → inject system cols → fingerprint.
pub fn fingerprint_from_json_schema(json_schema: &str) -> Result<DatasetFingerprint, DatasetError> {
    let schema = json_schema_to_arrow(json_schema)?;
    let schema_with_sys = inject_system_columns(schema)?;
    schema_fingerprint(&schema_with_sys)
}

/// Build registration inputs from a JSON Schema string + namespace + partition columns.
/// Returns `(arrow_schema, fingerprint)`.
#[allow(dead_code)]
pub(crate) fn build_registration(
    json_schema: &str,
    _namespace: &DatasetNamespace,
    _partition_columns: &[String],
) -> Result<(Schema, DatasetFingerprint), DatasetError> {
    let schema = json_schema_to_arrow(json_schema)?;
    let schema_with_sys = inject_system_columns(schema)?;
    let fingerprint = schema_fingerprint(&schema_with_sys)?;
    Ok((schema_with_sys, fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_schema_json() -> &'static str {
        r#"{
            "type": "object",
            "title": "UserEvent",
            "properties": {
                "user_id": {"type": "string"},
                "event_type": {"type": "string"},
                "value": {"type": "number"},
                "count": {"type": "integer"},
                "active": {"type": "boolean"},
                "score": {"type": "number"}
            },
            "required": ["user_id", "event_type", "value", "count", "active"]
        }"#
    }

    fn optional_schema_json() -> &'static str {
        r#"{
            "type": "object",
            "title": "OptionalModel",
            "properties": {
                "name": {"type": "string"},
                "age": {"anyOf": [{"type": "integer"}, {"type": "null"}]},
                "score": {"anyOf": [{"type": "number"}, {"type": "null"}]}
            },
            "required": ["name"]
        }"#
    }

    fn nested_schema_json() -> &'static str {
        r##"{
            "type": "object",
            "title": "Order",
            "properties": {
                "order_id": {"type": "string"},
                "address": {"$ref": "#/$defs/Address"}
            },
            "required": ["order_id", "address"],
            "$defs": {
                "Address": {
                    "type": "object",
                    "properties": {
                        "street": {"type": "string"},
                        "city": {"type": "string"},
                        "zip": {"type": "string"}
                    },
                    "required": ["street", "city", "zip"]
                }
            }
        }"##
    }

    fn datetime_schema_json() -> &'static str {
        r#"{
            "type": "object",
            "title": "Event",
            "properties": {
                "created_at": {"type": "string", "format": "date-time"},
                "event_date": {"type": "string", "format": "date"},
                "label": {"type": "string"}
            },
            "required": ["created_at", "event_date", "label"]
        }"#
    }

    fn list_schema_json() -> &'static str {
        r#"{
            "type": "object",
            "title": "BatchPrediction",
            "properties": {
                "model_id": {"type": "string"},
                "scores": {"type": "array", "items": {"type": "number"}}
            },
            "required": ["model_id", "scores"]
        }"#
    }

    fn enum_schema_json() -> &'static str {
        r#"{
            "type": "object",
            "title": "Status",
            "properties": {
                "status": {"enum": ["active", "inactive", "pending"]},
                "name": {"type": "string"}
            },
            "required": ["status", "name"]
        }"#
    }

    fn list_of_nested_schema_json() -> &'static str {
        r##"{
            "type": "object",
            "title": "Report",
            "properties": {
                "report_id": {"type": "string"},
                "items": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/ReportItem"}
                }
            },
            "required": ["report_id", "items"],
            "$defs": {
                "ReportItem": {
                    "type": "object",
                    "properties": {
                        "label": {"type": "string"},
                        "value": {"type": "number"}
                    },
                    "required": ["label", "value"]
                }
            }
        }"##
    }

    #[test]
    fn test_flat_schema() {
        let schema = json_schema_to_arrow(flat_schema_json()).unwrap();
        assert_eq!(schema.fields().len(), 6);

        let user_id = schema.field_with_name("user_id").unwrap();
        assert_eq!(user_id.data_type(), &DataType::Utf8View);
        assert!(!user_id.is_nullable());

        // score is not in required, so nullable
        let score = schema.field_with_name("score").unwrap();
        assert!(score.is_nullable());

        let value = schema.field_with_name("value").unwrap();
        assert_eq!(value.data_type(), &DataType::Float64);

        let count = schema.field_with_name("count").unwrap();
        assert_eq!(count.data_type(), &DataType::Int64);

        let active = schema.field_with_name("active").unwrap();
        assert_eq!(active.data_type(), &DataType::Boolean);
    }

    #[test]
    fn test_optional_fields() {
        let schema = json_schema_to_arrow(optional_schema_json()).unwrap();

        let name = schema.field_with_name("name").unwrap();
        assert!(!name.is_nullable());
        assert_eq!(name.data_type(), &DataType::Utf8View);

        let age = schema.field_with_name("age").unwrap();
        assert!(age.is_nullable());
        assert_eq!(age.data_type(), &DataType::Int64);

        let score = schema.field_with_name("score").unwrap();
        assert!(score.is_nullable());
        assert_eq!(score.data_type(), &DataType::Float64);
    }

    #[test]
    fn test_nested_struct() {
        let schema = json_schema_to_arrow(nested_schema_json()).unwrap();

        let address = schema.field_with_name("address").unwrap();
        assert!(!address.is_nullable());
        assert!(matches!(address.data_type(), DataType::Struct(_)));

        if let DataType::Struct(fields) = address.data_type() {
            assert_eq!(fields.len(), 3);
            let street = fields.find("street").map(|(_, f)| f.clone());
            assert!(street.is_some());
            assert_eq!(street.unwrap().data_type(), &DataType::Utf8View);
        }
    }

    #[test]
    fn test_datetime_formats() {
        let schema = json_schema_to_arrow(datetime_schema_json()).unwrap();

        let created = schema.field_with_name("created_at").unwrap();
        assert!(matches!(
            created.data_type(),
            DataType::Timestamp(TimeUnit::Microsecond, _)
        ));

        let date = schema.field_with_name("event_date").unwrap();
        assert_eq!(date.data_type(), &DataType::Date32);
    }

    #[test]
    fn test_list_type() {
        let schema = json_schema_to_arrow(list_schema_json()).unwrap();

        let scores = schema.field_with_name("scores").unwrap();
        assert!(matches!(scores.data_type(), DataType::List(_)));
        if let DataType::List(item) = scores.data_type() {
            assert_eq!(item.data_type(), &DataType::Float64);
        }
    }

    #[test]
    fn test_enum_type() {
        let schema = json_schema_to_arrow(enum_schema_json()).unwrap();

        let status = schema.field_with_name("status").unwrap();
        assert!(matches!(status.data_type(), DataType::Dictionary(_, _)));
    }

    #[test]
    fn test_list_of_nested() {
        let schema = json_schema_to_arrow(list_of_nested_schema_json()).unwrap();

        let items = schema.field_with_name("items").unwrap();
        assert!(matches!(items.data_type(), DataType::List(_)));
        if let DataType::List(item_field) = items.data_type() {
            assert!(matches!(item_field.data_type(), DataType::Struct(_)));
        }
    }

    #[test]
    fn test_system_columns_injected() {
        let schema = json_schema_to_arrow(flat_schema_json()).unwrap();
        let schema = inject_system_columns(schema).unwrap();

        let created = schema.field_with_name(SCOUTER_CREATED_AT).unwrap();
        assert!(matches!(
            created.data_type(),
            DataType::Timestamp(TimeUnit::Microsecond, _)
        ));
        assert!(!created.is_nullable());

        let partition_date = schema.field_with_name(SCOUTER_PARTITION_DATE).unwrap();
        assert_eq!(partition_date.data_type(), &DataType::Date32);
        assert!(!partition_date.is_nullable());

        let batch_id = schema.field_with_name(SCOUTER_BATCH_ID).unwrap();
        assert_eq!(batch_id.data_type(), &DataType::Utf8);
        assert!(!batch_id.is_nullable());
    }

    #[test]
    fn test_reserved_column_collision_error() {
        let bad = r#"{
            "type": "object",
            "properties": {
                "scouter_created_at": {"type": "string"}
            },
            "required": ["scouter_created_at"]
        }"#;
        let schema = json_schema_to_arrow(bad).unwrap();
        let err = inject_system_columns(schema).unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
        assert!(err.to_string().contains("reserved"));
    }

    #[test]
    fn test_fingerprint_stability() {
        let fp1 = fingerprint_from_json_schema(flat_schema_json()).unwrap();
        let fp2 = fingerprint_from_json_schema(flat_schema_json()).unwrap();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_changes_on_field_add() {
        let fp1 = fingerprint_from_json_schema(flat_schema_json()).unwrap();

        let modified = r#"{
            "type": "object",
            "title": "UserEvent",
            "properties": {
                "user_id": {"type": "string"},
                "event_type": {"type": "string"},
                "value": {"type": "number"},
                "count": {"type": "integer"},
                "active": {"type": "boolean"},
                "score": {"type": "number"},
                "new_field": {"type": "string"}
            },
            "required": ["user_id", "event_type", "value", "count", "active"]
        }"#;
        let fp2 = fingerprint_from_json_schema(modified).unwrap();
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_is_32_chars() {
        let fp = fingerprint_from_json_schema(flat_schema_json()).unwrap();
        assert_eq!(fp.as_str().len(), 32);
    }

    #[test]
    fn test_fingerprint_field_order_independent() {
        // Same fields, different declaration order → same fingerprint
        let schema_a = r#"{
            "type": "object",
            "properties": {
                "alpha": {"type": "string"},
                "beta": {"type": "integer"}
            },
            "required": ["alpha", "beta"]
        }"#;
        let schema_b = r#"{
            "type": "object",
            "properties": {
                "beta": {"type": "integer"},
                "alpha": {"type": "string"}
            },
            "required": ["alpha", "beta"]
        }"#;
        let fp_a = fingerprint_from_json_schema(schema_a).unwrap();
        let fp_b = fingerprint_from_json_schema(schema_b).unwrap();
        assert_eq!(fp_a, fp_b);
    }

    #[test]
    fn test_unsupported_type_error() {
        let bad = r#"{
            "type": "object",
            "properties": {
                "field": {"type": "unknown_type"}
            },
            "required": ["field"]
        }"#;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::UnsupportedType(_)));
    }

    #[test]
    fn test_missing_ref_error() {
        let bad = r##"{
            "type": "object",
            "properties": {
                "nested": {"$ref": "#/$defs/NonExistent"}
            },
            "required": ["nested"]
        }"##;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::RefResolutionError(_)));
    }

    #[test]
    fn test_missing_properties_key_error() {
        let bad = r#"{"type": "object"}"#;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
    }

    #[test]
    fn test_bad_ref_format_error() {
        let bad = r##"{
            "type": "object",
            "properties": {
                "x": {"$ref": "definitions/Foo"}
            },
            "required": ["x"]
        }"##;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::RefResolutionError(_)));
    }

    #[test]
    fn test_property_not_object_error() {
        let bad = r#"{
            "type": "object",
            "properties": {
                "x": true
            },
            "required": ["x"]
        }"#;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
    }

    #[test]
    fn test_any_of_multiple_non_null_variants_error() {
        let bad = r#"{
            "type": "object",
            "properties": {
                "x": {"anyOf": [{"type": "integer"}, {"type": "string"}]}
            },
            "required": ["x"]
        }"#;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::UnsupportedType(_)));
    }

    #[test]
    fn test_any_of_null_enum_encoding() {
        // Pydantic v2 may encode Optional[T] null branch as {"enum": [null]}
        let schema = r#"{
            "type": "object",
            "properties": {
                "x": {"anyOf": [{"type": "integer"}, {"enum": [null]}]}
            },
            "required": []
        }"#;
        let result = json_schema_to_arrow(schema);
        assert!(result.is_ok());
        let field = result.unwrap();
        let x = field.field_with_name("x").unwrap();
        assert!(x.is_nullable());
        assert_eq!(x.data_type(), &DataType::Int64);
    }

    #[test]
    fn test_any_of_const_null_encoding() {
        // Pydantic v2 may encode null branch as {"const": null}
        let schema = r#"{
            "type": "object",
            "properties": {
                "x": {"anyOf": [{"type": "string"}, {"const": null}]}
            },
            "required": []
        }"#;
        let result = json_schema_to_arrow(schema);
        assert!(result.is_ok());
        let field = result.unwrap();
        let x = field.field_with_name("x").unwrap();
        assert!(x.is_nullable());
        assert_eq!(x.data_type(), &DataType::Utf8View);
    }

    #[test]
    fn test_free_form_dict_is_unsupported_type() {
        let bad = r#"{
            "type": "object",
            "properties": {
                "x": {"type": "object"}
            },
            "required": ["x"]
        }"#;
        let err = json_schema_to_arrow(bad).unwrap_err();
        assert!(matches!(err, DatasetError::UnsupportedType(_)));
    }

    #[test]
    fn test_build_registration_includes_sys_cols() {
        use crate::dataset::types::DatasetNamespace;
        let ns = DatasetNamespace::new("cat", "sch", "tbl").unwrap();
        let (schema, fingerprint) = build_registration(flat_schema_json(), &ns, &[]).unwrap();
        assert!(schema.index_of(SCOUTER_CREATED_AT).is_ok());
        assert!(schema.index_of(SCOUTER_PARTITION_DATE).is_ok());
        assert!(schema.index_of(SCOUTER_BATCH_ID).is_ok());
        assert_eq!(fingerprint.as_str().len(), 32);
    }

    #[test]
    fn test_max_depth_exceeded() {
        // Build a deeply nested $ref chain that exceeds MAX_SCHEMA_DEPTH
        // We simulate by crafting a schema where $defs reference each other > 32 levels deep.
        // Since $ref resolves via a flat $defs lookup (no actual recursion in the JSON),
        // we test the depth by constructing an "object" with nested properties 33 levels deep.
        let mut inner = r#"{"type": "string"}"#.to_string();
        for _ in 0..MAX_SCHEMA_DEPTH {
            inner = format!(
                r#"{{"type": "object", "properties": {{"x": {inner}}}, "required": ["x"]}}"#
            );
        }
        let schema = format!(
            r#"{{"type": "object", "properties": {{"root": {inner}}}, "required": ["root"]}}"#
        );
        let err = json_schema_to_arrow(&schema).unwrap_err();
        assert!(matches!(err, DatasetError::SchemaParseError(_)));
        assert!(err.to_string().contains("depth"));
    }
}
