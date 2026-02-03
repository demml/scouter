use chrono::{DateTime, Duration, Utc};
use pyo3::pyfunction;
use scouter_types::sql::TraceSpan;

#[cfg(feature = "server")]
use scouter_types::trace::Attribute;

#[cfg(feature = "server")]
use serde_json::{json, Value};

#[cfg(feature = "server")]
pub struct SpanBuilder {
    trace_id: String,
    service_name: String,
    root_span_id: String,
    current_time: DateTime<Utc>,
    next_span_id: u32,
    next_order: i32,
}

#[cfg(feature = "server")]
impl SpanBuilder {
    pub fn new(trace_id: impl Into<String>, service_name: impl Into<String>) -> Self {
        let trace_id = trace_id.into();
        Self {
            root_span_id: "span_0".to_string(),
            trace_id,
            service_name: service_name.into(),
            current_time: Utc::now(),
            next_span_id: 0,
            next_order: 0,
        }
    }

    fn next_id(&mut self) -> String {
        let id = format!("span_{}", self.next_span_id);
        self.next_span_id += 1;
        id
    }

    fn next_order(&mut self) -> i32 {
        let order = self.next_order;
        self.next_order += 1;
        order
    }

    pub fn create_span(
        &mut self,
        name: impl Into<String>,
        parent_id: Option<String>,
        depth: i32,
        duration_ms: i64,
        status_code: i32,
    ) -> TraceSpan {
        let span_id = self.next_id();
        let start_time = self.current_time;
        let end_time = start_time + Duration::milliseconds(duration_ms);
        self.current_time = end_time;

        let path = if let Some(ref parent) = parent_id {
            vec![parent.clone(), span_id.clone()]
        } else {
            vec![span_id.clone()]
        };

        TraceSpan {
            trace_id: self.trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id: parent_id,
            span_name: name.into(),
            span_kind: Some("INTERNAL".to_string()),
            start_time,
            end_time: Some(end_time),
            duration_ms: Some(duration_ms),
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
            root_span_id: self.root_span_id.clone(),
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

#[pyfunction]
pub fn create_simple_trace() -> Vec<TraceSpan> {
    #[cfg(feature = "server")]
    {
        let mut builder = SpanBuilder::new("trace_001", "test_service");

        vec![
            builder.create_span("root", None, 0, 100, 1),
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
        let mut builder = SpanBuilder::new("trace_002", "nested_service");

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
        let mut builder = SpanBuilder::new("trace_003", "error_service");

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
        let mut builder = SpanBuilder::new("trace_004", "attribute_service");

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
        let mut builder_a = SpanBuilder::new("trace_006", "service_a");
        let mut builder_b = SpanBuilder::new("trace_006", "service_b");
        let mut builder_c = SpanBuilder::new("trace_006", "service_c");

        builder_a.root_span_id = "span_0".to_string();
        builder_b.root_span_id = "span_0".to_string();
        builder_c.root_span_id = "span_0".to_string();

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
        let mut builder = SpanBuilder::new("trace_007", "pattern_service");

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
