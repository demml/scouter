use chrono::{DateTime, Duration, Utc};
use pyo3::pyfunction;
use rand::Rng;
use scouter_types::sql::TraceSpan;
use scouter_types::TagRecord;
use scouter_types::SCOUTER_ENTITY;
use scouter_types::{TraceRecord, TraceSpanRecord};
const SCOPE: &str = "scope";
const UID: &str = "test";

#[cfg(feature = "server")]
use scouter_types::{trace::Attribute, SpanId, TraceId};

#[cfg(feature = "server")]
use serde_json::{json, Value};

#[cfg(feature = "server")]
type TraceRecords = (TraceRecord, Vec<TraceSpanRecord>, Vec<TagRecord>);

#[cfg(feature = "server")]
fn create_trace_id_from_str(id: &str) -> TraceId {
    let mut bytes = [0u8; 16];
    let id_bytes = id.as_bytes();
    let len = id_bytes.len().min(16);
    bytes[..len].copy_from_slice(&id_bytes[..len]);
    TraceId::from_bytes(bytes)
}

#[cfg(feature = "server")]
fn create_span_id_from_str(id: &str) -> SpanId {
    // convert the string to bytes, ensuring it's 8 bytes long (padding or truncating as needed)
    let mut bytes = [0u8; 8];
    let id_bytes = id.as_bytes();
    let len = id_bytes.len().min(8);
    bytes[..len].copy_from_slice(&id_bytes[..len]);
    SpanId::from_bytes(bytes)
}

#[cfg(feature = "server")]
pub struct SpanBuilder {
    trace_id: TraceId,
    service_name: String,
    root_span_id: SpanId,
    current_time: DateTime<Utc>,
    next_span_id: u32,
    next_order: i32,
}

#[cfg(feature = "server")]
impl SpanBuilder {
    pub fn new(trace_id: TraceId, service_name: impl Into<String>) -> Self {
        Self {
            root_span_id: create_span_id_from_str("span_0"),
            trace_id,
            service_name: service_name.into(),
            current_time: Utc::now(),
            next_span_id: 0,
            next_order: 0,
        }
    }

    fn next_span(&mut self) -> SpanId {
        let id = format!("span_{}", self.next_span_id);
        self.next_span_id += 1;
        create_span_id_from_str(&id)
    }

    fn next_order(&mut self) -> i32 {
        let order = self.next_order;
        self.next_order += 1;
        order
    }

    pub fn create_span(
        &mut self,
        name: impl Into<String>,
        parent_span_id: Option<String>,
        depth: i32,
        duration_ms: i64,
        status_code: i32,
    ) -> TraceSpan {
        let next_span = self.next_span();
        let start_time = self.current_time;
        let end_time = start_time + Duration::milliseconds(duration_ms);
        self.current_time = end_time;

        let parent_span =
            parent_span_id.map(|parent_span_id| create_span_id_from_str(&parent_span_id));

        let path = if let Some(ref parent) = parent_span {
            vec![parent.to_hex(), next_span.to_hex()]
        } else {
            vec![next_span.to_hex()]
        };

        TraceSpan {
            trace_id: self.trace_id.to_hex(),
            span_id: next_span.to_hex(),
            parent_span_id: parent_span.map(|s| s.to_hex()),
            span_name: name.into(),
            span_kind: Some("INTERNAL".to_string()),
            start_time,
            end_time,
            duration_ms,
            status_code,
            status_message: if status_code == 2 {
                Some("Error occurred".to_string())
            } else {
                None
            },
            attributes: vec![],
            events: vec![],
            links: vec![],
            depth,
            path,
            root_span_id: self.root_span_id.to_hex(),
            service_name: self.service_name.clone(),
            span_order: self.next_order(),
            input: None,
            output: None,
        }
    }

    pub fn with_attributes(mut span: TraceSpan, attrs: Vec<(&str, Value)>) -> TraceSpan {
        span.attributes = attrs
            .into_iter()
            .map(|(k, v)| Attribute {
                key: k.to_string(),
                value: v,
            })
            .collect();
        span
    }

    pub fn with_error(mut span: TraceSpan, message: impl Into<String>) -> TraceSpan {
        span.status_code = 2;
        span.status_message = Some(message.into());
        span
    }
}

pub fn create_simple_trace_no_py() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let trace_id = create_trace_id_from_str("trace_001");
        let mut builder = SpanBuilder::new(trace_id, "test_service");

        let root = SpanBuilder::with_attributes(
            builder.create_span("root", None, 0, 150, 1),
            vec![
                ("http.method", json!("POST")),
                ("http.status_code", json!(200)),
                ("http.url", json!("https://api.example.com/users")),
            ],
        );

        vec![
            root,
            builder.create_span("child_1", Some("span_0".to_string()), 1, 50, 1),
            builder.create_span("child_2", Some("span_0".to_string()), 1, 30, 1),
        ]
    }
    #[cfg(not(feature = "server"))]
    {
        tracing::warn!(
            "create_simple_trace is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_simple_trace() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let trace_id = create_trace_id_from_str("trace_001");
        let mut builder = SpanBuilder::new(trace_id, "test_service");

        let root = SpanBuilder::with_attributes(
            builder.create_span("root", None, 0, 150, 1),
            vec![
                ("http.method", json!("POST")),
                ("http.status_code", json!(200)),
                ("http.url", json!("https://api.example.com/users")),
            ],
        );

        vec![
            root,
            builder.create_span("child_1", Some("span_0".to_string()), 1, 50, 1),
            builder.create_span("child_2", Some("span_0".to_string()), 1, 30, 1),
        ]
    }
    #[cfg(not(feature = "server"))]
    {
        tracing::warn!(
            "create_simple_trace is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_nested_trace() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder = SpanBuilder::new(create_trace_id_from_str("trace_002"), "nested_service");

        let root = builder.create_span("init", None, 0, 300, 1);
        let process = builder.create_span("process", Some("span_0".to_string()), 1, 200, 1);
        let db_query = builder.create_span("db_query", Some("span_1".to_string()), 2, 100, 1);
        let finalize = builder.create_span("finalize", Some("span_1".to_string()), 2, 50, 1);

        vec![root, process, db_query, finalize]
    }
    #[cfg(not(feature = "server"))]
    {
        tracing::warn!(
            "create_nested_trace is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_trace_with_errors() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder = SpanBuilder::new(create_trace_id_from_str("trace_003"), "error_service");

        let root = builder.create_span("root", None, 0, 200, 1);
        let failing_span = SpanBuilder::with_error(
            builder.create_span("failing_operation", Some("span_0".to_string()), 1, 100, 2),
            "Connection timeout",
        );
        let recovery = builder.create_span("recovery", Some("span_0".to_string()), 1, 50, 1);

        vec![root, failing_span, recovery]
    }

    #[cfg(not(feature = "server"))]
    {
        tracing::warn!(
            "create_trace_with_errors is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_trace_with_attributes() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder =
            SpanBuilder::new(create_trace_id_from_str("trace_004"), "attribute_service");

        let root = SpanBuilder::with_attributes(
            builder.create_span("root", None, 0, 150, 1),
            vec![
                ("http.method", json!("POST")),
                ("http.status_code", json!(200)),
                ("http.url", json!("https://api.example.com/users")),
            ],
        );

        let api_call = SpanBuilder::with_attributes(
            builder.create_span("api_call", Some("span_0".to_string()), 1, 100, 1),
            vec![
                ("model", json!("gpt-4")),
                ("tokens.input", json!(150)),
                ("tokens.output", json!(300)),
                ("cost", json!(0.045)),
                ("response", json!({"success": true, "data": {"id": 12345}})),
            ],
        );

        vec![root, api_call]
    }

    #[cfg(not(feature = "server"))]
    {
        use tracing::warn;

        tracing::warn!(
            "create_trace_with_attributes is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_multi_service_trace() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder_a = SpanBuilder::new(create_trace_id_from_str("trace_006"), "service_a");
        let mut builder_b = SpanBuilder::new(create_trace_id_from_str("trace_006"), "service_b");
        let mut builder_c = SpanBuilder::new(create_trace_id_from_str("trace_006"), "service_c");

        builder_a.root_span_id = create_span_id_from_str("span_0");
        builder_b.root_span_id = create_span_id_from_str("span_0");
        builder_c.root_span_id = create_span_id_from_str("span_0");

        vec![
            builder_a.create_span("gateway", None, 0, 300, 1),
            {
                builder_b.next_span_id = 1;
                builder_b.create_span("auth_check", Some("span_0".to_string()), 1, 50, 1)
            },
            {
                builder_c.next_span_id = 2;
                builder_c.create_span("data_fetch", Some("span_0".to_string()), 1, 200, 1)
            },
        ]
    }
    #[cfg(not(feature = "server"))]
    {
        use tracing::warn;

        tracing::warn!(
            "create_multi_service_trace is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[pyfunction]
pub fn create_sequence_pattern_trace() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder =
            SpanBuilder::new(create_trace_id_from_str("trace_007"), "pattern_service");

        vec![
            builder.create_span("start", None, 0, 50, 1),
            builder.create_span("call_tool", Some("span_0".to_string()), 1, 100, 1),
            builder.create_span("run_agent", Some("span_1".to_string()), 2, 150, 1),
            builder.create_span("call_tool", Some("span_2".to_string()), 3, 80, 1),
            builder.create_span("run_agent", Some("span_3".to_string()), 4, 120, 1),
            builder.create_span("finish", Some("span_4".to_string()), 5, 30, 1),
        ]
    }
    #[cfg(not(feature = "server"))]
    {
        tracing::warn!(
            "create_sequence_pattern_trace is not available without the 'server' feature enabled."
        );
        vec![]
    }
}

#[cfg(feature = "server")]
fn random_trace_record() -> TraceRecord {
    let mut rng = rand::rng();
    let random_num = rng.random_range(0..1000);
    let trace_id = TraceId::from_bytes(rng.random::<[u8; 16]>());
    let span_id = SpanId::from_bytes(rng.random::<[u8; 8]>());
    let created_at = Utc::now() + chrono::Duration::milliseconds(random_num);

    TraceRecord {
        trace_id,
        created_at,
        service_name: format!("service_{}", random_num % 10),
        scope_name: SCOPE.to_string(),
        scope_version: None,
        trace_state: "running".to_string(),
        start_time: created_at,
        end_time: created_at + chrono::Duration::milliseconds(150),
        duration_ms: 150,
        status_code: 0,
        span_count: 1,
        status_message: "OK".to_string(),
        root_span_id: span_id,
        tags: vec![],
        process_attributes: vec![],
    }
}

#[cfg(feature = "server")]
fn random_span_record(
    trace_id: &TraceId,
    parent_span_id: Option<&SpanId>,
    service_name: &str,
    minutes_offset: i64,
) -> TraceSpanRecord {
    let mut rng = rand::rng();
    let span_id = SpanId::from_bytes(rng.random::<[u8; 8]>());

    let random_offset_ms = rng.random_range(0..1000);
    let duration_ms_val = rng.random_range(50..500);

    let created_at = Utc::now() - Duration::minutes(minutes_offset)
        + chrono::Duration::milliseconds(random_offset_ms);
    let start_time = created_at;
    let end_time = start_time + chrono::Duration::milliseconds(duration_ms_val);

    let status_code = if rng.random_bool(0.95) { 0 } else { 2 };
    let span_kind_options = ["SERVER", "CLIENT", "INTERNAL", "PRODUCER", "CONSUMER"];
    let span_kind = span_kind_options[rng.random_range(0..span_kind_options.len())].to_string();
    let mut attributes = vec![];

    // randomly add SCOUTER_ENTITY to attributes based on 30% chance
    if rng.random_bool(0.3) {
        attributes.push(Attribute {
            key: SCOUTER_ENTITY.to_string(),
            value: Value::String(UID.to_string()),
        });
    } else {
        attributes.push(Attribute {
            key: "random_attribute".to_string(),
            value: Value::String(format!("value_{}", rng.random_range(0..100))),
        });
    }

    if rng.random_bool(0.1) {
        attributes.push(Attribute {
            key: "component".to_string(),
            value: Value::String("kafka".to_string()),
        });
    }

    TraceSpanRecord {
        created_at,
        span_id,
        trace_id: trace_id.clone(),
        parent_span_id: parent_span_id.cloned(),
        flags: 1,
        trace_state: String::new(),
        service_name: service_name.to_string(),
        scope_name: SCOPE.to_string(),
        scope_version: None,
        span_name: format!("random_operation_{}", rng.random_range(0..10)),
        span_kind,
        start_time,
        end_time,
        duration_ms: duration_ms_val,
        status_code,
        status_message: if status_code == 2 {
            "Internal Server Error".to_string()
        } else {
            "OK".to_string()
        },
        attributes,
        events: vec![],
        links: vec![],
        label: None,
        input: Value::default(),
        output: Value::default(),
        resource_attributes: vec![],
    }
}

#[cfg(feature = "server")]
pub fn generate_trace_with_spans(num_spans: usize, minutes_offset: i64) -> TraceRecords {
    use scouter_types::TagRecord;

    let trace_record = random_trace_record();
    let mut spans: Vec<TraceSpanRecord> = Vec::new();
    let mut rng = rand::rng();
    let mut tag_records: Vec<TagRecord> = Vec::new();

    for i in 0..num_spans {
        let parent_span_id = if i == 0 {
            None
        } else {
            Some(&spans[rng.random_range(0..spans.len())].span_id)
        };
        let span_record = random_span_record(
            &trace_record.trace_id,
            parent_span_id,
            &trace_record.service_name,
            minutes_offset,
        );
        spans.push(span_record);
    }

    // get first trace

    let tag_record = TagRecord {
        entity_type: "trace".to_string(),
        entity_id: trace_record.trace_id.to_hex(),
        key: "scouter.queue.record".to_string(),
        value: trace_record.trace_id.to_hex(),
    };

    tag_records.push(tag_record);

    (trace_record, spans, tag_records)
}
