use chrono::{DateTime, Duration, Utc};
use scouter_types::sql::TraceSpan;
use scouter_types::{trace::Attribute, SpanId, TraceId};
use serde_json::{json, Value};

fn create_trace_id_from_str(id: &str) -> TraceId {
    let mut bytes = [0u8; 16];
    let id_bytes = id.as_bytes();
    let len = id_bytes.len().min(16);
    bytes[..len].copy_from_slice(&id_bytes[..len]);
    TraceId::from_bytes(bytes)
}

fn create_span_id_from_str(id: &str) -> SpanId {
    // convert the string to bytes, ensuring it's 8 bytes long (padding or truncating as needed)
    let mut bytes = [0u8; 8];
    let id_bytes = id.as_bytes();
    let len = id_bytes.len().min(8);
    bytes[..len].copy_from_slice(&id_bytes[..len]);
    SpanId::from_bytes(bytes)
}

pub struct SpanBuilder {
    trace_id: TraceId,
    service_name: String,
    root_span_id: SpanId,
    current_time: DateTime<Utc>,
    next_span_id: u32,
    next_order: i32,
}

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
}

pub fn create_simple_trace() -> Vec<TraceSpan> {
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
