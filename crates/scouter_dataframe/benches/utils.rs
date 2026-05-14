#![allow(dead_code)]

use crate::tiers::{BenchArtifact, ObjectStoreCountSnapshot, SpanMetric, registration_or_default};
use scouter_mocks::{generate_trace_with_entity, generate_trace_with_spans};
use scouter_types::TraceSpanRecord;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Subscriber, warn};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

const OBJECT_STORE_SPAN_NAME: &str = "object_store.request";
const OBJECT_STORE_OPERATION_ATTR: &str = "object_store.operation";

static BENCH_SPAN_COLLECTOR: OnceLock<BenchSpanCollector> = OnceLock::new();

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

#[derive(Clone, Debug)]
pub struct BenchSpanCollector {
    records: Arc<Mutex<Vec<SpanRecord>>>,
}

#[derive(Clone, Debug)]
pub struct SpanRecord {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub duration_ns: u64,
}

#[derive(Debug)]
struct SpanTiming {
    name: String,
    attrs: Vec<(String, String)>,
    start: Instant,
}

#[derive(Default)]
struct AttrVisitor {
    attrs: Vec<(String, String)>,
}

impl Visit for AttrVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.attrs
            .push((field.name().to_string(), format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.attrs
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.attrs
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.attrs
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.attrs
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.attrs
            .push((field.name().to_string(), value.to_string()));
    }
}

impl BenchSpanCollector {
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn records(&self) -> Vec<SpanRecord> {
        self.records
            .lock()
            .expect("bench span collector mutex poisoned")
            .clone()
    }

    pub fn records_len(&self) -> usize {
        self.records
            .lock()
            .expect("bench span collector mutex poisoned")
            .len()
    }

    pub fn records_since(&self, start: usize) -> Vec<SpanRecord> {
        self.records
            .lock()
            .expect("bench span collector mutex poisoned")
            .iter()
            .skip(start)
            .cloned()
            .collect()
    }

    pub fn summary(&self) -> BTreeMap<String, SpanMetric> {
        summarize_spans(&self.records())
    }

    pub fn object_store_counts_since(&self, start: usize) -> ObjectStoreCountSnapshot {
        object_store_counts(&self.records_since(start))
    }
}

pub fn install_bench_span_collector() -> BenchSpanCollector {
    BENCH_SPAN_COLLECTOR
        .get_or_init(|| {
            let collector = BenchSpanCollector::new();
            let _ = tracing_subscriber::registry()
                .with(collector.clone())
                .try_init();
            collector
        })
        .clone()
}

impl<S> Layer<S> for BenchSpanCollector
where
    S: Subscriber,
    S: for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        let mut visitor = AttrVisitor::default();
        attrs.record(&mut visitor);
        span.extensions_mut().insert(SpanTiming {
            name: span.metadata().name().to_string(),
            attrs: visitor.attrs,
            start: Instant::now(),
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        let mut extensions = span.extensions_mut();
        let Some(timing) = extensions.get_mut::<SpanTiming>() else {
            return;
        };

        let mut visitor = AttrVisitor::default();
        values.record(&mut visitor);
        timing.attrs.extend(visitor.attrs);
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };

        let Some(timing) = span.extensions_mut().remove::<SpanTiming>() else {
            return;
        };

        let duration_ns = timing.start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.records
            .lock()
            .expect("bench span collector mutex poisoned")
            .push(SpanRecord {
                name: timing.name,
                attrs: timing.attrs,
                duration_ns,
            });
    }
}

pub fn summarize_spans(records: &[SpanRecord]) -> BTreeMap<String, SpanMetric> {
    let mut by_name: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for record in records {
        by_name
            .entry(record.name.clone())
            .or_default()
            .push(record.duration_ns / 1_000);
    }

    by_name
        .into_iter()
        .map(|(name, mut values)| {
            values.sort_unstable();
            let count = values.len() as u64;
            let p50_us = percentile_u64(&values, 50.0);
            let p95_us = percentile_u64(&values, 95.0);
            let p99_us = percentile_u64(&values, 99.0);
            let sum_us = values.iter().sum();
            (
                name,
                SpanMetric {
                    count,
                    p50_us,
                    p95_us,
                    p99_us,
                    sum_us,
                },
            )
        })
        .collect()
}

pub fn span_metric_from_samples(samples_us: &[u64]) -> SpanMetric {
    if samples_us.is_empty() {
        return SpanMetric::default();
    }

    let mut values = samples_us.to_vec();
    values.sort_unstable();
    SpanMetric {
        count: values.len() as u64,
        p50_us: percentile_u64(&values, 50.0),
        p95_us: percentile_u64(&values, 95.0),
        p99_us: percentile_u64(&values, 99.0),
        sum_us: values.iter().sum(),
    }
}

pub fn object_store_counts(records: &[SpanRecord]) -> ObjectStoreCountSnapshot {
    let mut counts = ObjectStoreCountSnapshot::default();
    for record in records
        .iter()
        .filter(|record| record.name == OBJECT_STORE_SPAN_NAME)
    {
        match attr_value(record, OBJECT_STORE_OPERATION_ATTR).as_deref() {
            Some("list") => counts.list += 1,
            Some("list_with_delimiter") => counts.list_with_delimiter += 1,
            Some("head") => counts.head += 1,
            Some("get") => counts.get += 1,
            Some("get_range") => counts.get_range += 1,
            Some("put") => counts.put += 1,
            Some("delete") => counts.delete += 1,
            Some("copy") => counts.copy += 1,
            _ => {}
        }
    }
    counts
}

fn attr_value(record: &SpanRecord, key: &str) -> Option<String> {
    record
        .attrs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.trim_matches('"').to_string())
}

fn percentile_u64(values: &[u64], percentile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = ((percentile / 100.0) * values.len() as f64) as usize;
    values[index.min(values.len() - 1)]
}

#[allow(clippy::too_many_arguments)]
pub fn write_bench_artifact(
    bench_binary: &'static str,
    group_name: &'static str,
    actual_runtime: Duration,
    spans: BTreeMap<String, SpanMetric>,
    object_store_counts: ObjectStoreCountSnapshot,
    refresh_on_request_path_total: u64,
    query_entrypoint: Option<&'static str>,
    result_rows: Option<u64>,
) {
    let registration = registration_or_default(bench_binary, group_name);
    let artifact = BenchArtifact {
        commit: current_commit(),
        bench_group: group_name.to_string(),
        tier: registration.tier.as_u8(),
        blocking: registration.tier.as_u8() == 0,
        scenario_class: registration.scenario_class.to_string(),
        runtime_budget_secs: registration.runtime_budget_secs,
        actual_runtime_secs: actual_runtime.as_secs_f64(),
        fixture_rows: registration.fixture_rows,
        fixture_spans: registration.fixture_spans,
        storage_profile: registration.storage_profile.to_string(),
        query_entrypoint: query_entrypoint.map(str::to_string),
        result_rows,
        spans,
        object_store_counts,
        refresh_on_request_path_total,
    };

    if let Err(err) = write_artifact(group_name, &artifact) {
        warn!(error = %err, bench_group = group_name, "failed to write bench artifact");
    }
}

fn write_artifact(group_name: &str, artifact: &BenchArtifact) -> Result<(), String> {
    let dir = target_metrics_dir();
    fs::create_dir_all(&dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    let path = dir.join(format!("{group_name}.json"));
    let json = serde_json::to_string_pretty(artifact)
        .map_err(|err| format!("failed to serialize bench artifact: {err}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn target_metrics_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map(|root| root.join("target").join("bench_metrics"))
        .unwrap_or_else(|| PathBuf::from("target").join("bench_metrics"))
}

fn current_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::registry::Registry;

    #[test]
    fn span_collector_summarizes_closed_spans() {
        let collector = BenchSpanCollector::new();
        let subscriber = Registry::default().with(collector.clone());

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("df.collect", rows = 10_u64);
            let _guard = span.enter();
        });

        let summary = collector.summary();
        let metric = summary.get("df.collect").unwrap();
        assert_eq!(metric.count, 1);
        assert!(metric.sum_us <= metric.p99_us || metric.count == 1);
    }

    #[test]
    fn span_summary_percentiles_are_stable() {
        let records = [1_u64, 2, 3, 4, 5]
            .into_iter()
            .map(|duration_us| SpanRecord {
                name: "delta.snapshot.refresh".to_string(),
                attrs: Vec::new(),
                duration_ns: duration_us * 1_000,
            })
            .collect::<Vec<_>>();

        let summary = summarize_spans(&records);
        let metric = summary.get("delta.snapshot.refresh").unwrap();
        assert_eq!(metric.count, 5);
        assert_eq!(metric.p50_us, 3);
        assert_eq!(metric.p95_us, 5);
        assert_eq!(metric.p99_us, 5);
        assert_eq!(metric.sum_us, 15);
    }

    #[test]
    fn object_store_counts_are_derived_from_span_attrs() {
        let records = vec![
            SpanRecord {
                name: "object_store.request".to_string(),
                attrs: vec![("object_store.operation".to_string(), "list".to_string())],
                duration_ns: 1_000,
            },
            SpanRecord {
                name: "object_store.request".to_string(),
                attrs: vec![(
                    "object_store.operation".to_string(),
                    "\"get_range\"".to_string(),
                )],
                duration_ns: 1_000,
            },
            SpanRecord {
                name: "df.collect".to_string(),
                attrs: Vec::new(),
                duration_ns: 1_000,
            },
        ];

        let counts = object_store_counts(&records);

        assert_eq!(counts.list, 1);
        assert_eq!(counts.get_range, 1);
        assert_eq!(counts.total_operations(), 2);
    }
}
