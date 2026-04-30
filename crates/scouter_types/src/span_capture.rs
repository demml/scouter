use crate::TraceId as ScouterTraceId;
use crate::trace::{SCOUTER_EVAL_RUN_ID_ATTR, SCOUTER_EVAL_SCENARIO_ID_ATTR, TraceSpanRecord};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};
use tracing::{debug, warn};

pub const CAPTURE_BUFFER_MAX: usize = 20_000;

/// Whether any scoped local span capture is enabled.
pub static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Run-scoped buffers of captured spans.
pub static CAPTURE_BUFFERS: LazyLock<RwLock<HashMap<String, Vec<TraceSpanRecord>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Returns `true` if any local span capture scope is currently enabled.
pub fn is_capturing() -> bool {
    CAPTURING.load(Ordering::Acquire)
}

fn refresh_capturing(buffers: &HashMap<String, Vec<TraceSpanRecord>>) {
    CAPTURING.store(!buffers.is_empty(), Ordering::Release);
}

fn span_attr<'a>(span: &'a TraceSpanRecord, key: &str) -> Option<&'a str> {
    span.attributes
        .iter()
        .find(|attr| attr.key == key)
        .and_then(|attr| attr.value.as_str())
}

const MAX_CAPTURE_RUN_ID_LEN: usize = 256;

/// Enable scoped capture for a single evaluation run.
pub fn enable_capture(capture_run_id: &str) {
    if capture_run_id.len() > MAX_CAPTURE_RUN_ID_LEN {
        warn!(
            "capture_run_id exceeds {} chars and was rejected; capture not enabled",
            MAX_CAPTURE_RUN_ID_LEN
        );
        return;
    }
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    buffers.insert(capture_run_id.to_string(), Vec::new());
    refresh_capturing(&buffers);
}

/// Disable scoped capture for a run and discard any remaining buffered spans.
pub fn disable_capture(capture_run_id: &str) {
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    if let Some(discarded) = buffers.remove(capture_run_id)
        && !discarded.is_empty()
    {
        warn!(
            capture_run_id,
            "disable_local_capture: discarding {} buffered spans",
            discarded.len()
        );
        let trace_ids: Vec<String> = discarded
            .iter()
            .map(|span| span.trace_id.to_string())
            .collect();
        debug!(
            capture_run_id,
            trace_ids = ?trace_ids,
            "discarded buffered span trace IDs"
        );
    }
    refresh_capturing(&buffers);
}

/// Disable every scoped capture buffer.
pub fn disable_all_captures() {
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    buffers.clear();
    refresh_capturing(&buffers);
}

/// Buffer exported spans into their matching capture-run buffer.
///
/// Spans without `scouter.eval.run_id`, or spans for an inactive run, are dropped
/// while scoped local capture is active.
pub fn buffer_captured_spans(spans: Vec<TraceSpanRecord>) {
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    if buffers.is_empty() {
        refresh_capturing(&buffers);
        return;
    }

    let mut dropped = 0usize;
    for span in spans {
        let Some(capture_run_id) = span_attr(&span, SCOUTER_EVAL_RUN_ID_ATTR).map(str::to_string)
        else {
            dropped += 1;
            continue;
        };

        let Some(buffer) = buffers.get_mut(&capture_run_id) else {
            dropped += 1;
            continue;
        };

        if buffer.len() >= CAPTURE_BUFFER_MAX {
            warn!(
                capture_run_id,
                "scoped capture buffer full ({} records); dropping new span to prevent OOM",
                CAPTURE_BUFFER_MAX
            );
            dropped += 1;
            continue;
        }

        buffer.push(span);
    }

    if dropped > 0 {
        debug!(
            "scoped local capture dropped {} span(s) without an active {} attribute",
            dropped, SCOUTER_EVAL_RUN_ID_ATTR
        );
    }
    refresh_capturing(&buffers);
}

/// Drain all captured spans for one run (takes ownership).
pub fn drain_captured_spans(capture_run_id: &str) -> Vec<TraceSpanRecord> {
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    let drained = buffers
        .get_mut(capture_run_id)
        .map(std::mem::take)
        .unwrap_or_default();
    refresh_capturing(&buffers);
    drained
}

/// Returns clones of spans matching the given trace_ids for one run.
/// Does NOT drain the buffer.
pub fn get_captured_spans_by_trace_ids(
    capture_run_id: &str,
    trace_ids: &HashSet<ScouterTraceId>,
) -> Vec<TraceSpanRecord> {
    let buffers = CAPTURE_BUFFERS.read().unwrap_or_else(|p| p.into_inner());
    buffers
        .get(capture_run_id)
        .map(|buf| {
            buf.iter()
                .filter(|span| trace_ids.contains(&span.trace_id))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Returns a clone of all captured spans for one run without draining.
pub fn get_all_captured_spans(capture_run_id: &str) -> Vec<TraceSpanRecord> {
    CAPTURE_BUFFERS
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(capture_run_id)
        .cloned()
        .unwrap_or_default()
}

/// Returns clones of spans tagged with the given scenario_id without draining.
pub fn peek_spans_for_scenario(capture_run_id: &str, scenario_id: &str) -> Vec<TraceSpanRecord> {
    CAPTURE_BUFFERS
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(capture_run_id)
        .map(|buf| {
            buf.iter()
                .filter(|span| span_attr(span, SCOUTER_EVAL_SCENARIO_ID_ATTR) == Some(scenario_id))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Test-only helper for seeding scoped capture buffers.
#[cfg(test)]
pub fn push_captured_spans(capture_run_id: &str, spans: Vec<TraceSpanRecord>) {
    let mut buffers = CAPTURE_BUFFERS.write().unwrap_or_else(|p| p.into_inner());
    buffers
        .entry(capture_run_id.to_string())
        .or_default()
        .extend(spans);
    refresh_capturing(&buffers);
}

/// Drain the capture buffer for a run and bucket every span by scenario_id.
///
/// Spans are routed exclusively by the `scouter.eval.scenario_id` attribute.
/// Spans without a matching attribute are dropped.
///
/// Drains only `capture_run_id`, so concurrent eval runs cannot consume each
/// other's spans.
pub fn drain_and_group_spans_for_scenarios(
    capture_run_id: &str,
    scenario_ids: &HashSet<String>,
) -> HashMap<String, Vec<TraceSpanRecord>> {
    let drained = drain_captured_spans(capture_run_id);
    let mut grouped: HashMap<String, Vec<TraceSpanRecord>> = HashMap::new();
    let mut dropped = 0usize;

    for span in drained {
        match span_attr(&span, SCOUTER_EVAL_SCENARIO_ID_ATTR)
            .filter(|sid| scenario_ids.contains(*sid))
        {
            Some(sid) => {
                grouped.entry(sid.to_string()).or_default().push(span);
            }
            None => {
                dropped += 1;
            }
        }
    }

    if dropped > 0 {
        debug!(
            "drain_and_group_spans_for_scenarios: dropped {} span(s) without a matching {} attribute",
            dropped, SCOUTER_EVAL_SCENARIO_ID_ATTR
        );
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{Attribute, SpanEvent, SpanId, TraceId, TraceSpanRecord};
    use chrono::Utc;
    use serde_json::Value;

    const RUN_A: &str = "run_a";
    const RUN_B: &str = "run_b";

    fn clear_buffer(capture_run_id: &str) {
        let _ = drain_captured_spans(capture_run_id);
        disable_capture(capture_run_id);
    }

    fn push(run_id: &str, spans: Vec<TraceSpanRecord>) {
        push_captured_spans(run_id, spans);
    }

    fn make_span(
        trace_id: TraceId,
        span_seed: u8,
        name: &str,
        scenario_attr: Option<&str>,
    ) -> TraceSpanRecord {
        let now = Utc::now();
        let attributes = match scenario_attr {
            Some(sid) => vec![Attribute {
                key: SCOUTER_EVAL_SCENARIO_ID_ATTR.to_string(),
                value: Value::String(sid.to_string()),
            }],
            None => vec![],
        };
        TraceSpanRecord {
            created_at: now,
            trace_id,
            span_id: SpanId::from_bytes([span_seed; 8]),
            parent_span_id: None,
            span_name: name.to_string(),
            start_time: now,
            end_time: now,
            duration_ms: 1,
            attributes,
            events: Vec::<SpanEvent>::new(),
            service_name: "test".to_string(),
            ..Default::default()
        }
    }

    fn make_run_span(trace_id: TraceId, span_seed: u8, run_id: &str) -> TraceSpanRecord {
        let mut span = make_span(trace_id, span_seed, "run_span", None);
        span.attributes.push(Attribute {
            key: SCOUTER_EVAL_RUN_ID_ATTR.to_string(),
            value: Value::String(run_id.to_string()),
        });
        span
    }

    #[test]
    fn scoped_capture_isolates_run_buffers() {
        disable_all_captures();
        enable_capture(RUN_A);
        enable_capture(RUN_B);

        let trace_a = TraceId::from_bytes([1; 16]);
        let trace_b = TraceId::from_bytes([2; 16]);
        buffer_captured_spans(vec![
            make_run_span(trace_a, 1, RUN_A),
            make_run_span(trace_b, 2, RUN_B),
        ]);

        let run_a = drain_captured_spans(RUN_A);
        assert_eq!(run_a.len(), 1);
        assert_eq!(run_a[0].trace_id, trace_a);

        let run_b = drain_captured_spans(RUN_B);
        assert_eq!(run_b.len(), 1);
        assert_eq!(run_b[0].trace_id, trace_b);

        disable_all_captures();
    }

    #[test]
    fn scoped_capture_drops_unscoped_spans() {
        disable_all_captures();
        enable_capture(RUN_A);

        let trace = TraceId::from_bytes([3; 16]);
        buffer_captured_spans(vec![make_span(trace, 1, "missing_run", None)]);

        assert!(drain_captured_spans(RUN_A).is_empty());
        disable_all_captures();
    }

    #[test]
    fn drain_and_group_isolates_by_attribute() {
        clear_buffer(RUN_A);
        let trace_a = TraceId::from_bytes([1; 16]);
        let trace_b = TraceId::from_bytes([2; 16]);
        let shared = TraceId::from_bytes([9; 16]);

        push(
            RUN_A,
            vec![
                make_span(trace_a, 1, "wrapper_a", Some("a")),
                make_span(trace_b, 2, "wrapper_b", Some("b")),
                make_span(shared, 3, "child_a", Some("a")),
                make_span(shared, 4, "child_b", Some("b")),
            ],
        );

        let scenario_ids: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let grouped = drain_and_group_spans_for_scenarios(RUN_A, &scenario_ids);

        assert_eq!(grouped["a"].len(), 2);
        assert_eq!(grouped["b"].len(), 2);
        assert!(grouped["a"].iter().any(|s| s.span_name == "wrapper_a"));
        assert!(grouped["a"].iter().any(|s| s.span_name == "child_a"));
        assert!(grouped["b"].iter().any(|s| s.span_name == "child_b"));
        clear_buffer(RUN_A);
    }

    #[test]
    fn drain_and_group_orphan_skipped_when_no_attribute() {
        clear_buffer(RUN_A);
        let trace = TraceId::from_bytes([11; 16]);
        push(RUN_A, vec![make_span(trace, 1, "no_attr_no_hint", None)]);

        let scenario_ids: HashSet<String> = std::iter::once("a".to_string()).collect();
        let grouped = drain_and_group_spans_for_scenarios(RUN_A, &scenario_ids);
        assert!(grouped.is_empty());
        clear_buffer(RUN_A);
    }

    #[test]
    fn drain_and_group_drains_only_requested_run_buffer() {
        clear_buffer(RUN_A);
        clear_buffer(RUN_B);
        let trace_a = TraceId::from_bytes([12; 16]);
        let trace_b = TraceId::from_bytes([13; 16]);
        push(RUN_A, vec![make_span(trace_a, 1, "wrapper_a", Some("a"))]);
        push(RUN_B, vec![make_span(trace_b, 2, "wrapper_b", Some("b"))]);

        let scenario_ids: HashSet<String> = std::iter::once("a".to_string()).collect();
        let _ = drain_and_group_spans_for_scenarios(RUN_A, &scenario_ids);

        assert!(get_all_captured_spans(RUN_A).is_empty());
        assert_eq!(get_all_captured_spans(RUN_B).len(), 1);
        clear_buffer(RUN_B);
    }

    #[test]
    fn drain_and_group_unknown_scenario_attribute_falls_through() {
        clear_buffer(RUN_A);
        let trace = TraceId::from_bytes([13; 16]);
        push(
            RUN_A,
            vec![make_span(trace, 1, "wrapper_unknown", Some("z"))],
        );

        let scenario_ids: HashSet<String> = std::iter::once("a".to_string()).collect();
        let grouped = drain_and_group_spans_for_scenarios(RUN_A, &scenario_ids);
        assert!(grouped.is_empty());
        clear_buffer(RUN_A);
    }

    #[test]
    fn drain_and_group_orphan_span_without_attr_dropped() {
        clear_buffer(RUN_A);
        let shared = TraceId::from_bytes([14; 16]);
        push(
            RUN_A,
            vec![
                make_span(shared, 1, "wrapper_a", Some("a")),
                make_span(shared, 2, "wrapper_b", Some("b")),
                make_span(shared, 3, "child", None),
            ],
        );

        let scenario_ids: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let grouped = drain_and_group_spans_for_scenarios(RUN_A, &scenario_ids);

        assert_eq!(grouped["a"].len(), 1);
        assert_eq!(grouped["b"].len(), 1);
        clear_buffer(RUN_A);
    }
}
