#![allow(dead_code)]

use scouter_mocks::{generate_trace_with_entity, generate_trace_with_spans};
use scouter_types::TraceSpanRecord;
use std::time::Duration;

/// Create a simple 3-span trace as ingest records (ready for `write_spans()`).
pub fn _create_simple_trace() -> Vec<TraceSpanRecord> {
    let (_trace_record, spans, _tags) = generate_trace_with_spans(3, 0);
    spans
}

/// Create a batch of approximately `n_spans` records across multiple traces.
/// Uses 5 spans per trace for realistic nesting depth.
pub fn _create_trace_batch(n_spans: usize) -> Vec<TraceSpanRecord> {
    let spans_per_trace = 5;
    let n_traces = n_spans.div_ceil(spans_per_trace);
    (0..n_traces)
        .flat_map(|_| {
            let (_record, spans, _tags) = generate_trace_with_spans(spans_per_trace, 0);
            spans
        })
        .collect()
}

/// Create a batch of spans where every root span carries `entity_uid` as its
/// `scouter.entity` attribute, so the ingest pipeline populates `entity_ids`.
pub fn create_entity_trace_batch(n_traces: usize, entity_uid: &str) -> Vec<TraceSpanRecord> {
    (0..n_traces)
        .flat_map(|_| {
            let (_record, spans, _tags) = generate_trace_with_entity(5, entity_uid, 0);
            spans
        })
        .collect()
}

pub struct Percentiles {
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub count: usize,
}

pub fn compute_percentiles(mut timings: Vec<Duration>) -> Percentiles {
    assert!(!timings.is_empty(), "no timings to compute");
    timings.sort_unstable();
    let len = timings.len();
    let last = *timings.last().unwrap();
    let pct = |p: f64| timings[((p / 100.0) * len as f64) as usize].min(last);
    let mean = timings.iter().sum::<Duration>() / len as u32;
    Percentiles {
        p50: pct(50.0),
        p95: pct(95.0),
        p99: pct(99.0),
        min: *timings.first().unwrap(),
        max: last,
        mean,
        count: len,
    }
}

pub fn print_percentiles(label: &str, p: &Percentiles) {
    println!(
        "  {label:<45} n={count:>5}  p50={p50:>7.2}ms  p95={p95:>7.2}ms  p99={p99:>7.2}ms  min={min:.2}ms  max={max:.2}ms  mean={mean:.2}ms",
        label = label,
        count = p.count,
        p50 = p.p50.as_secs_f64() * 1000.0,
        p95 = p.p95.as_secs_f64() * 1000.0,
        p99 = p.p99.as_secs_f64() * 1000.0,
        min = p.min.as_secs_f64() * 1000.0,
        max = p.max.as_secs_f64() * 1000.0,
        mean = p.mean.as_secs_f64() * 1000.0,
    );
}
