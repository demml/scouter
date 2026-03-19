use crate::trace::{TraceSpanRecord, SCOUTER_EVAL_SCENARIO_ID_ATTR};
use crate::TraceId as ScouterTraceId;
use std::collections::{HashMap, HashSet};
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
    std::mem::take(&mut *CAPTURE_BUFFER.write().unwrap_or_else(|p| p.into_inner()))
}

/// Returns clones of spans matching the given trace_ids.
/// Does NOT drain the buffer.
pub fn get_captured_spans_by_trace_ids(
    trace_ids: &HashSet<ScouterTraceId>,
) -> Vec<TraceSpanRecord> {
    let buf = CAPTURE_BUFFER.read().unwrap_or_else(|p| p.into_inner());
    buf.iter()
        .filter(|span| trace_ids.contains(&span.trace_id))
        .cloned()
        .collect()
}

/// Returns a clone of all captured spans without draining.
pub fn get_all_captured_spans() -> Vec<TraceSpanRecord> {
    CAPTURE_BUFFER
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Two-pass buffer scan that groups all captured spans by `scouter.eval.scenario_id`.
///
/// **Pass 1**: Build a `trace_id → scenario_id` map from spans that carry the
/// `scouter.eval.scenario_id` attribute (i.e. the orchestrator's wrapper spans).
///
/// **Pass 2**: Group every span whose `trace_id` appears in that map into the
/// corresponding scenario bucket — this picks up child spans (e.g. LLM calls)
/// that share the trace but don't carry the attribute directly.
///
/// Does NOT drain the buffer.
pub fn get_spans_grouped_by_scenario_id(
    scenario_ids: &HashSet<String>,
) -> HashMap<String, Vec<TraceSpanRecord>> {
    let buf = CAPTURE_BUFFER.read().unwrap_or_else(|p| p.into_inner());

    // Pass 1: trace_id → scenario_id for spans that carry the attribute
    let mut trace_to_scenario: HashMap<ScouterTraceId, String> = HashMap::new();
    for span in buf.iter() {
        for attr in &span.attributes {
            if attr.key == SCOUTER_EVAL_SCENARIO_ID_ATTR {
                if let Some(sid) = attr.value.as_str() {
                    if scenario_ids.contains(sid) {
                        trace_to_scenario.insert(span.trace_id, sid.to_string());
                    }
                }
                break;
            }
        }
    }

    // Pass 2: group all spans (including children) by their scenario
    let mut grouped: HashMap<String, Vec<TraceSpanRecord>> = HashMap::new();
    for span in buf.iter() {
        if let Some(sid) = trace_to_scenario.get(&span.trace_id) {
            grouped.entry(sid.clone()).or_default().push(span.clone());
        }
    }
    grouped
}

/// Returns the set of trace IDs for any captured span that carries a
/// `scouter.eval.scenario_id` attribute whose value matches one of the
/// provided scenario IDs.
///
/// This covers the case where the user only records traces (no `EvalRecord`),
/// so trace IDs never appear on an `EvalRecord.trace_id` but *are* present on
/// the span itself as a span attribute set by the eval orchestrator.
pub fn get_trace_ids_by_scenario_ids(scenario_ids: &HashSet<String>) -> HashSet<ScouterTraceId> {
    let buf = CAPTURE_BUFFER.read().unwrap_or_else(|p| p.into_inner());
    buf.iter()
        .filter(|span| {
            span.attributes.iter().any(|attr| {
                attr.key == SCOUTER_EVAL_SCENARIO_ID_ATTR
                    && attr
                        .value
                        .as_str()
                        .map(|v| scenario_ids.contains(v))
                        .unwrap_or(false)
            })
        })
        .map(|span| span.trace_id)
        .collect()
}
