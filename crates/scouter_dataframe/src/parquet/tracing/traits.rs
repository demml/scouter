use arrow::datatypes::*;
use deltalake::kernel::{
    DataType as DeltaDataType, PrimitiveType, StructField as DeltaStructField, StructType,
};
use std::sync::Arc;

pub(crate) fn attribute_field() -> Field {
    Field::new(
        "attributes",
        DataType::Map(
            Arc::new(Field::new(
                "key_value",
                DataType::Struct(
                    vec![
                        Field::new("key", DataType::Utf8, false),
                        Field::new("value", DataType::Utf8View, true),
                    ]
                    .into(),
                ),
                false,
            )),
            false,
        ),
        false,
    )
}

/// Map field for resource_attributes (nullable map, matching attribute_field structure)
pub(crate) fn resource_attribute_field() -> Field {
    Field::new(
        "resource_attributes",
        DataType::Map(
            Arc::new(Field::new(
                "key_value",
                DataType::Struct(
                    vec![
                        Field::new("key", DataType::Utf8, false),
                        Field::new("value", DataType::Utf8View, true),
                    ]
                    .into(),
                ),
                false,
            )),
            false,
        ),
        true, // nullable: a span may have no resource attributes
    )
}

pub trait TraceSchemaExt {
    /// Define the Arrow schema for trace spans.
    ///
    /// Hierarchy fields (depth, span_order, path, root_span_id) are NOT stored —
    /// they are computed at query time via Rust DFS traversal, matching how Jaeger/Zipkin operate.
    ///
    /// Fields align 1:1 with `TraceSpanRecord` (the ingest type), enabling zero-transform writes.
    fn create_schema() -> Schema {
        Schema::new(vec![
            // ========== Core Identifiers ==========
            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
            Field::new("span_id", DataType::FixedSizeBinary(8), false),
            Field::new("parent_span_id", DataType::FixedSizeBinary(8), true),
            // ========== W3C Trace Context ==========
            Field::new("flags", DataType::Int32, false),
            Field::new("trace_state", DataType::Utf8, false),
            // ========== Instrumentation Scope ==========
            Field::new("scope_name", DataType::Utf8, false),
            Field::new("scope_version", DataType::Utf8, true),
            // ========== Metadata ==========
            // Dictionary encoding for high-repetition string fields
            Field::new(
                "service_name",
                DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new("span_name", DataType::Utf8, false),
            Field::new(
                "span_kind",
                DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
                true,
            ),
            // ========== Temporal Data ==========
            Field::new(
                "start_time",
                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                false,
            ),
            Field::new(
                "end_time",
                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                false,
            ),
            Field::new("duration_ms", DataType::Int64, false),
            // ========== Status ==========
            Field::new("status_code", DataType::Int32, false),
            Field::new("status_message", DataType::Utf8, true),
            // ========== Scouter-specific ==========
            Field::new("label", DataType::Utf8, true),
            // ========== Attributes ==========
            attribute_field(),
            resource_attribute_field(),
            // ========== Events (Nested) ==========
            // SpanEvent: all fields non-nullable, attributes Vec can be empty
            Field::new(
                "events",
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(
                        vec![
                            Field::new("name", DataType::Utf8, false),
                            Field::new(
                                "timestamp",
                                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                                false,
                            ),
                            attribute_field(),
                            Field::new("dropped_attributes_count", DataType::UInt32, false),
                        ]
                        .into(),
                    ),
                    true,
                ))),
                false,
            ),
            // ========== Links (Nested) ==========
            Field::new(
                "links",
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(
                        vec![
                            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
                            Field::new("span_id", DataType::FixedSizeBinary(8), false),
                            Field::new("trace_state", DataType::Utf8, false),
                            attribute_field(),
                            Field::new("dropped_attributes_count", DataType::UInt32, false),
                        ]
                        .into(),
                    ),
                    true,
                ))),
                false,
            ),
            // ========== Payload (Large JSON) ==========
            Field::new("input", DataType::Utf8View, true),
            Field::new("output", DataType::Utf8View, true),
            // ========== Full-Text Search Optimization ==========
            // Pre-computed concatenated search string to avoid JSON parsing at query time
            Field::new("search_blob", DataType::Utf8View, false),
            // ========== Partitioning ==========
            // Hive-style date partition key derived from start_time — lets DataFusion skip
            Field::new("partition_date", DataType::Date32, false),
        ])
    }
}

/// Convert Arrow Schema to Delta Lake StructFields
pub fn arrow_schema_to_delta(schema: &Schema) -> Vec<DeltaStructField> {
    schema
        .fields()
        .iter()
        .map(|field| arrow_field_to_delta(field))
        .collect()
}

/// Convert a single Arrow Field to Delta Lake StructField
fn arrow_field_to_delta(field: &Field) -> DeltaStructField {
    let delta_type = arrow_type_to_delta(field.data_type());
    DeltaStructField::new(field.name().clone(), delta_type, field.is_nullable())
}

/// Map Arrow DataType to Delta Lake DataType
fn arrow_type_to_delta(arrow_type: &DataType) -> DeltaDataType {
    match arrow_type {
        // Primitive types
        DataType::Boolean => DeltaDataType::Primitive(PrimitiveType::Boolean),
        DataType::Int8 => DeltaDataType::Primitive(PrimitiveType::Byte),
        DataType::Int16 => DeltaDataType::Primitive(PrimitiveType::Short),
        DataType::Int32 => DeltaDataType::Primitive(PrimitiveType::Integer),
        DataType::Int64 => DeltaDataType::Primitive(PrimitiveType::Long),
        // Unsigned int types — Delta Lake has no native unsigned; map to next-larger signed type
        DataType::UInt8 | DataType::UInt16 => DeltaDataType::Primitive(PrimitiveType::Short),
        DataType::UInt32 => DeltaDataType::Primitive(PrimitiveType::Integer),
        DataType::UInt64 => DeltaDataType::Primitive(PrimitiveType::Long),
        DataType::Float32 => DeltaDataType::Primitive(PrimitiveType::Float),
        DataType::Float64 => DeltaDataType::Primitive(PrimitiveType::Double),

        // String types
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {
            DeltaDataType::Primitive(PrimitiveType::String)
        }

        // Binary types
        DataType::Binary | DataType::LargeBinary | DataType::FixedSizeBinary(_) => {
            DeltaDataType::Primitive(PrimitiveType::Binary)
        }

        // Temporal types
        DataType::Timestamp(TimeUnit::Microsecond, Some(_))
        | DataType::Timestamp(TimeUnit::Nanosecond, Some(_)) => {
            DeltaDataType::Primitive(PrimitiveType::Timestamp)
        }
        DataType::Timestamp(TimeUnit::Microsecond, None)
        | DataType::Timestamp(TimeUnit::Nanosecond, None) => {
            DeltaDataType::Primitive(PrimitiveType::TimestampNtz)
        }
        DataType::Date32 | DataType::Date64 => DeltaDataType::Primitive(PrimitiveType::Date),

        // Complex types
        DataType::List(field) | DataType::LargeList(field) => {
            let element_type = arrow_type_to_delta(field.data_type());
            DeltaDataType::Array(Box::new(deltalake::kernel::ArrayType::new(
                element_type,
                field.is_nullable(),
            )))
        }

        DataType::Struct(fields) => {
            let delta_fields: Vec<DeltaStructField> =
                fields.iter().map(|f| arrow_field_to_delta(f)).collect();
            DeltaDataType::Struct(Box::new(StructType::try_new(delta_fields).unwrap()))
        }

        DataType::Map(field, _sorted) => {
            if let DataType::Struct(map_fields) = field.data_type() {
                let key_type = arrow_type_to_delta(map_fields[0].data_type());
                let value_type = arrow_type_to_delta(map_fields[1].data_type());
                DeltaDataType::Map(Box::new(deltalake::kernel::MapType::new(
                    key_type,
                    value_type,
                    map_fields[1].is_nullable(),
                )))
            } else {
                DeltaDataType::Primitive(PrimitiveType::String)
            }
        }

        // Dictionary encoding - use underlying value type
        DataType::Dictionary(_, value_type) => arrow_type_to_delta(value_type),

        // Fallback for unsupported types
        _ => DeltaDataType::Primitive(PrimitiveType::String),
    }
}
