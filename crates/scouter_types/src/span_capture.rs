use crate::trace::TraceSpanRecord;
use crate::TraceId as ScouterTraceId;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

pub const CAPTURE_BUFFER_MAX: usize = 20_000;

/// Whether local span capture is enabled.
pub static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Global buffer of captured spans.
pub static CAPTURE_BUFFER: RwLock<Vec<TraceSpanRecord>> = RwLock::new(Vec::new());

/// Returns `true` if local span capture is currently enabled.
pub fn is_capturing() -> bool {
    CAPTURING.load(Ordering::Acquire)
}

/// Drain all captured spans from the buffer (takes ownership).
pub fn drain_captured_spans() -> Vec<TraceSpanRecord> {
    std::mem::take(&mut *CAPTURE_BUFFER.write().unwrap())
}

/// Returns clones of spans matching the given trace_ids.
/// Does NOT drain the buffer.
pub fn get_captured_spans_by_trace_ids(
    trace_ids: &HashSet<ScouterTraceId>,
) -> Vec<TraceSpanRecord> {
    let buf = CAPTURE_BUFFER.read().unwrap();
    buf.iter()
        .filter(|span| trace_ids.contains(&span.trace_id))
        .cloned()
        .collect()
}

/// Returns a clone of all captured spans without draining.
pub fn get_all_captured_spans() -> Vec<TraceSpanRecord> {
    CAPTURE_BUFFER.read().unwrap().clone()
}
