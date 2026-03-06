use scouter_mocks::generate_trace_with_spans;
use scouter_types::TraceSpanRecord;

/// Create a simple 3-span trace as ingest records (ready for `write_spans()`).
pub fn create_simple_trace() -> Vec<TraceSpanRecord> {
    let (_trace_record, spans, _tags) = generate_trace_with_spans(3, 0);
    spans
}
