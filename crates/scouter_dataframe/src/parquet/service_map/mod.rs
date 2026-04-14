use chrono::DateTime;
use regex::Regex;
use std::sync::OnceLock;

pub const CATALOG: &str = "scouter";
pub const SCHEMA: &str = "service_map";
pub const TABLE: &str = "connections";

const MAX_ENDPOINT_LEN: usize = 256;

static UUID_RE: OnceLock<Regex> = OnceLock::new();
static INT_ID_RE: OnceLock<Regex> = OnceLock::new();
static TRACEPARENT_RE: OnceLock<Regex> = OnceLock::new();

fn uuid_re() -> &'static Regex {
    UUID_RE.get_or_init(|| {
        Regex::new(
            r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        )
        .expect("UUID regex is valid")
    })
}

fn int_id_re() -> &'static Regex {
    INT_ID_RE.get_or_init(|| Regex::new(r"^\d+$").expect("int ID regex is valid"))
}

fn traceparent_re() -> &'static Regex {
    TRACEPARENT_RE.get_or_init(|| {
        Regex::new(r"(?i)^[0-9a-f]{2}-([0-9a-f]{32})-[0-9a-f]{16}-[0-9a-f]{2}$")
            .expect("traceparent regex is valid")
    })
}

/// Strip UUIDs and integer path segments to prevent cardinality explosion.
///
/// `/users/12345/orders/abc-def` → `/users/{id}/orders/{id}`
pub fn normalize_endpoint(path: &str) -> String {
    let normalized = path
        .split('/')
        .map(|seg| {
            if uuid_re().is_match(seg) || int_id_re().is_match(seg) {
                "{id}"
            } else {
                seg
            }
        })
        .collect::<Vec<_>>()
        .join("/");

    normalized.chars().take(MAX_ENDPOINT_LEN).collect()
}

/// Extract the trace ID from a W3C `traceparent` header value.
///
/// Returns `None` if the header is absent, malformed, or empty.
pub fn extract_trace_id(traceparent: &str) -> Option<String> {
    traceparent_re()
        .captures(traceparent.trim())
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Inspect a JSON object body and return a JSON-encoded `{field: type}` map.
///
/// Returns `None` if the body is not valid JSON or is not a top-level object.
pub fn infer_schema(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let obj = value.as_object()?;
    let type_map: serde_json::Map<String, serde_json::Value> = obj
        .iter()
        .map(|(k, v)| {
            let type_name = match v {
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Object(_) => "object",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Null => "null",
            };
            (k.clone(), serde_json::Value::String(type_name.to_string()))
        })
        .collect();
    serde_json::to_string(&type_map).ok()
}

/// Validate that a string is safe to embed as a single-quoted SQL string literal.
///
/// Allows alphanumeric characters, underscores, and hyphens — the character set
/// used by real service names (e.g. `recommendation-api`). Safe because the value
/// is always interpolated inside single quotes; this is NOT safe for unquoted SQL
/// identifier contexts (column/table names).
// TODO: Replace string interpolation with DataFusion parameterized queries in a future pass.
fn sanitize_string_filter_value(s: &str) -> Result<&str, String> {
    if s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        Ok(s)
    } else {
        Err(
            "Invalid service_name: must contain only alphanumeric characters, underscores, or hyphens"
                .to_string(),
        )
    }
}

/// Build the topology aggregation SQL for the service map connections table.
///
/// All parameters are optional — omitting all returns the full graph.
/// - `service_name`: filters to a specific `destination_service`
/// - `since`: RFC 3339 timestamp lower bound on `scouter_created_at`
pub fn build_topology_sql(
    service_name: Option<&str>,
    since: Option<&str>,
) -> Result<String, String> {
    let mut predicates: Vec<String> = Vec::new();

    if let Some(svc) = service_name {
        let svc = sanitize_string_filter_value(svc)?;
        predicates.push(format!("destination_service = '{svc}'"));
    }

    if let Some(s) = since {
        // RFC 3339 is structurally incapable of containing SQL-hostile characters;
        // chrono rejects anything non-conforming.
        // TODO: replace with DataFusion parameterized queries.
        DateTime::parse_from_rfc3339(s)
            .map_err(|e| format!("Invalid 'since' datetime: {e}"))?;
        predicates.push(format!("scouter_created_at >= '{s}'"));
    }

    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", predicates.join(" AND "))
    };

    Ok(format!(
        "SELECT \
            source_service, \
            destination_service, \
            endpoint, \
            COUNT(*) AS total_calls, \
            MIN(scouter_created_at) AS first_seen, \
            MAX(scouter_created_at) AS last_seen, \
            AVG(latency_ms) AS avg_latency_ms, \
            SUM(CASE WHEN error = true THEN 1 ELSE 0 END) AS error_count, \
            ROUND(CAST(SUM(CASE WHEN error = true THEN 1 ELSE 0 END) AS DOUBLE) \
                * 100.0 / CAST(COUNT(*) AS DOUBLE), 2) AS error_rate_pct \
         FROM {CATALOG}.{SCHEMA}.{TABLE} \
         {where_clause} \
         GROUP BY source_service, destination_service, endpoint \
         ORDER BY total_calls DESC"
    ))
}

/// A single edge in the service topology graph, produced by `batches_to_edges`.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ServiceGraphEdge {
    pub source_service: String,
    pub destination_service: String,
    pub endpoint: String,
    pub total_calls: i64,
    pub first_seen: String,
    pub last_seen: String,
    pub avg_latency_ms: f64,
    pub error_count: i64,
    pub error_rate_pct: f64,
}

/// Deserialize Arrow `RecordBatch`es from a topology query into `ServiceGraphEdge`s.
pub fn batches_to_edges(
    batches: &[arrow_array::RecordBatch],
) -> Result<Vec<ServiceGraphEdge>, String> {
    let mut edges = Vec::new();

    for batch in batches {
        let n = batch.num_rows();
        if n == 0 {
            continue;
        }

        let schema = batch.schema();

        macro_rules! col_idx {
            ($name:expr) => {
                schema
                    .index_of($name)
                    .map_err(|_| format!("Missing column '{}'", $name))?
            };
        }

        let src_col = col_idx!("source_service");
        let dst_col = col_idx!("destination_service");
        let ep_col = col_idx!("endpoint");
        let calls_col = col_idx!("total_calls");
        let first_col = col_idx!("first_seen");
        let last_col = col_idx!("last_seen");
        let lat_col = col_idx!("avg_latency_ms");
        let errc_col = col_idx!("error_count");
        let errr_col = col_idx!("error_rate_pct");

        for i in 0..n {
            let source_service = extract_string(batch.column(src_col).as_ref(), i)?;
            let destination_service = extract_string(batch.column(dst_col).as_ref(), i)?;
            let endpoint = extract_string(batch.column(ep_col).as_ref(), i)?;
            // DataFusion guarantees non-null for COUNT(*) and AVG over non-empty groups
            let total_calls = extract_i64(batch.column(calls_col).as_ref(), i)?;
            let first_seen = extract_timestamp_str(batch.column(first_col).as_ref(), i)?;
            let last_seen = extract_timestamp_str(batch.column(last_col).as_ref(), i)?;
            let avg_latency_ms = extract_f64(batch.column(lat_col).as_ref(), i)?;
            let error_count = extract_i64(batch.column(errc_col).as_ref(), i)?;
            let error_rate_pct = extract_f64(batch.column(errr_col).as_ref(), i)?;

            edges.push(ServiceGraphEdge {
                source_service,
                destination_service,
                endpoint,
                total_calls,
                first_seen,
                last_seen,
                avg_latency_ms,
                error_count,
                error_rate_pct,
            });
        }
    }

    Ok(edges)
}

fn extract_string(col: &dyn arrow_array::Array, i: usize) -> Result<String, String> {
    use arrow::datatypes::DataType;
    use arrow_array::{DictionaryArray, StringArray};

    match col.data_type() {
        DataType::Utf8 => col
            .as_any()
            .downcast_ref::<StringArray>()
            .map(|a| a.value(i).to_string())
            .ok_or_else(|| format!("Failed to read Utf8 at {i}")),
        DataType::Dictionary(_, _) => {
            let dict = col
                .as_any()
                .downcast_ref::<DictionaryArray<arrow_array::types::Int32Type>>()
                .ok_or_else(|| format!("Expected Dictionary(Int32, _) at {i}"))?;
            let values = dict
                .values()
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| "Dict values not StringArray".to_string())?;
            let key = dict.key(i).ok_or_else(|| format!("Null dict key at {i}"))?;
            Ok(values.value(key).to_string())
        }
        other => Err(format!("Unexpected string column type: {other}")),
    }
}

fn extract_i64(col: &dyn arrow_array::Array, i: usize) -> Result<i64, String> {
    use arrow_array::Int64Array;
    col.as_any()
        .downcast_ref::<Int64Array>()
        .map(|a| a.value(i))
        .ok_or_else(|| format!("Expected Int64 at {i}"))
}

fn extract_f64(col: &dyn arrow_array::Array, i: usize) -> Result<f64, String> {
    use arrow_array::Float64Array;
    col.as_any()
        .downcast_ref::<Float64Array>()
        .map(|a| a.value(i))
        .ok_or_else(|| format!("Expected Float64 at {i}"))
}

fn extract_timestamp_str(col: &dyn arrow_array::Array, i: usize) -> Result<String, String> {
    use arrow_array::TimestampMicrosecondArray;
    let ts = col
        .as_any()
        .downcast_ref::<TimestampMicrosecondArray>()
        .map(|a| a.value(i))
        .ok_or_else(|| format!("Expected TimestampMicrosecond at {i}"))?;
    let dt = DateTime::<chrono::Utc>::from_timestamp_micros(ts)
        .ok_or_else(|| format!("Invalid timestamp micros: {ts}"))?;
    Ok(dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_uuid_and_int() {
        assert_eq!(
            normalize_endpoint("/users/12345/orders"),
            "/users/{id}/orders"
        );
        assert_eq!(
            normalize_endpoint("/models/abc123de-f456-7890-abcd-ef1234567890/predict"),
            "/models/{id}/predict"
        );
        assert_eq!(normalize_endpoint("/health"), "/health");
        assert_eq!(
            normalize_endpoint("/v1/api/42/items/99"),
            "/v1/api/{id}/items/{id}"
        );
    }

    #[test]
    fn normalize_strips_at_max_len() {
        let segment = "a".repeat(300);
        let path = format!("/{segment}");
        let result = normalize_endpoint(&path);
        assert!(result.chars().count() <= MAX_ENDPOINT_LEN);
    }

    #[test]
    fn normalize_preserves_alphanumeric_slug() {
        assert_eq!(
            normalize_endpoint("/api/abc-123def/data"),
            "/api/abc-123def/data"
        );
    }

    #[test]
    fn extract_trace_id_valid() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert_eq!(
            extract_trace_id(header),
            Some("4bf92f3577b34da6a3ce929d0e0e4736".to_string())
        );
    }

    #[test]
    fn extract_trace_id_invalid() {
        assert_eq!(extract_trace_id("invalid"), None);
        assert_eq!(extract_trace_id(""), None);
    }

    #[test]
    fn infer_schema_object() {
        let body = br#"{"user_id": "abc", "score": 0.9, "count": 5, "active": true}"#;
        let result = infer_schema(body).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["user_id"], "string");
        assert_eq!(parsed["score"], "number");
        assert_eq!(parsed["count"], "number");
        assert_eq!(parsed["active"], "boolean");
    }

    #[test]
    fn infer_schema_nested_types() {
        let body = br#"{"meta": {}, "items": [], "label": null}"#;
        let result = infer_schema(body).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["meta"], "object");
        assert_eq!(parsed["items"], "array");
        assert_eq!(parsed["label"], "null");
    }

    #[test]
    fn infer_schema_rejects_non_object() {
        assert!(infer_schema(b"not json").is_none());
        assert!(infer_schema(b"[1, 2, 3]").is_none());
    }

    #[test]
    fn build_topology_sql_no_filters() {
        let sql = build_topology_sql(None, None).unwrap();
        assert!(!sql.contains("WHERE"));
        assert!(sql.contains("GROUP BY source_service, destination_service, endpoint"));
        assert!(sql.contains("COUNT(*)"));
    }

    #[test]
    fn build_topology_sql_service_filter() {
        let sql = build_topology_sql(Some("my-svc"), None).unwrap();
        assert!(sql.contains("WHERE destination_service = 'my-svc'"));
    }

    #[test]
    fn build_topology_sql_since_filter() {
        let since = "2024-01-01T00:00:00Z";
        let sql = build_topology_sql(None, Some(since)).unwrap();
        assert!(sql.contains(&format!("WHERE scouter_created_at >= '{since}'")));
    }

    #[test]
    fn build_topology_sql_both_filters() {
        let since = "2024-01-01T00:00:00Z";
        let sql = build_topology_sql(Some("svc-b"), Some(since)).unwrap();
        assert!(sql.contains("destination_service = 'svc-b'"));
        assert!(sql.contains(&format!("scouter_created_at >= '{since}'")));
        assert!(sql.contains(" AND "));
    }

    #[test]
    fn build_topology_sql_rejects_invalid_service_name() {
        assert!(build_topology_sql(Some("'; DROP TABLE"), None).is_err());
        assert!(build_topology_sql(Some("svc b"), None).is_err());
    }

    #[test]
    fn build_topology_sql_rejects_invalid_since() {
        assert!(build_topology_sql(None, Some("yesterday")).is_err());
        assert!(build_topology_sql(None, Some("not-a-date")).is_err());
    }

    #[test]
    fn batches_to_edges_parses_correct_batch() {
        use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
        use arrow_array::{
            Float64Array, Int64Array, RecordBatch, StringArray, TimestampMicrosecondArray,
        };
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![
            Field::new("source_service", DataType::Utf8, false),
            Field::new("destination_service", DataType::Utf8, false),
            Field::new("endpoint", DataType::Utf8, false),
            Field::new("total_calls", DataType::Int64, false),
            Field::new(
                "first_seen",
                DataType::Timestamp(TimeUnit::Microsecond, None),
                false,
            ),
            Field::new(
                "last_seen",
                DataType::Timestamp(TimeUnit::Microsecond, None),
                false,
            ),
            Field::new("avg_latency_ms", DataType::Float64, false),
            Field::new("error_count", DataType::Int64, false),
            Field::new("error_rate_pct", DataType::Float64, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["svc-a"])),
                Arc::new(StringArray::from(vec!["svc-b"])),
                Arc::new(StringArray::from(vec!["/health"])),
                Arc::new(Int64Array::from(vec![3i64])),
                Arc::new(TimestampMicrosecondArray::from(vec![1_700_000_000_000_000i64])),
                Arc::new(TimestampMicrosecondArray::from(vec![1_700_000_001_000_000i64])),
                Arc::new(Float64Array::from(vec![12.5f64])),
                Arc::new(Int64Array::from(vec![0i64])),
                Arc::new(Float64Array::from(vec![0.0f64])),
            ],
        )
        .unwrap();

        let edges = batches_to_edges(&[batch]).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source_service, "svc-a");
        assert_eq!(edges[0].destination_service, "svc-b");
        assert_eq!(edges[0].endpoint, "/health");
        assert_eq!(edges[0].total_calls, 3);
        assert_eq!(edges[0].avg_latency_ms, 12.5);
    }

    #[test]
    fn batches_to_edges_missing_column() {
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow_array::{RecordBatch, StringArray};
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![
            Field::new("source_service", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![Arc::new(StringArray::from(vec!["svc-a"]))],
        )
        .unwrap();

        let result = batches_to_edges(&[batch]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing column"));
    }
}
