use chrono::Utc;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use scouter_types::error::RecordError;
use scouter_types::traits::RecordExt;
use scouter_types::RecordType;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceRecord {
    pub trace_id: String,
    pub space: String,
    pub name: String,
    pub version: String,
    pub drift_type: String,
    pub service_name: String,
    pub trace_state: String,
    pub start_time: chrono::DateTime<Utc>,
    pub end_time: chrono::DateTime<Utc>,
    pub duration_ms: i64,
    pub status: String,
    pub root_span_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceSpanRecord {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: String,
    pub space: String,
    pub name: String,
    pub version: String,
    pub drift_type: String,
    pub service_name: String,
    pub operation_name: String,
    pub span_kind: String,
    pub start_time: chrono::DateTime<Utc>,
    pub end_time: chrono::DateTime<Utc>,
    pub duration_ms: i64,
    pub status_code: String,
    pub status_message: String,
    pub attributes: Value,
    pub events: Value,
    pub links: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceBaggageRecord {
    pub trace_id: String,
    pub service_name: String,
    pub key: String,
    pub value: String,
    pub space: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceServerRecord {
    pub request: ExportTraceServiceRequest,
}

impl RecordExt for TraceServerRecord {
    fn record_type(&self) -> Result<RecordType, RecordError> {
        Ok(RecordType::Trace)
    }
}
