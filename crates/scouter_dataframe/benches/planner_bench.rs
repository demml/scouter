mod tiers;

use chrono::Utc;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use scouter_dataframe::parquet::tracing::queries::TraceQueries;
use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{
    Attribute, FilterClause, SpanId, StorageType, TraceId, TraceMetricsRequest, TraceSpanRecord,
};
use serde_json::json;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

const PLANNER_BENCH_SPANS: usize = 1_000_000;
const WRITE_CHUNK_SIZE: usize = 50_000;

fn storage_settings(uri: String) -> ObjectStorageSettings {
    ObjectStorageSettings {
        storage_uri: uri,
        storage_type: StorageType::Local,
        region: "us-east-1".to_string(),
        trace_compaction_interval_hours: 999,
        trace_flush_interval_secs: 1,
        trace_refresh_interval_secs: 10,
    }
}

fn trace_id(i: usize) -> TraceId {
    TraceId::from_bytes((i as u128).to_be_bytes())
}

fn span_id(i: usize, span: usize) -> SpanId {
    SpanId::from_bytes(((i * 10 + span) as u64).to_be_bytes())
}

fn make_span(i: usize, service: &str, planner_value: &str) -> TraceSpanRecord {
    let start =
        Utc::now() - chrono::Duration::minutes(5) + chrono::Duration::milliseconds(i as i64);
    TraceSpanRecord {
        created_at: start,
        trace_id: trace_id(i),
        span_id: span_id(i, 0),
        parent_span_id: None,
        flags: 1,
        trace_state: String::new(),
        scope_name: "planner.bench".to_string(),
        scope_version: Some("1".to_string()),
        span_name: "planner_op".to_string(),
        span_kind: "internal".to_string(),
        start_time: start,
        end_time: start + chrono::Duration::milliseconds(25),
        duration_ms: 25,
        status_code: 0,
        status_message: "OK".to_string(),
        attributes: vec![Attribute {
            key: "planner".to_string(),
            value: json!(planner_value),
        }],
        events: Vec::new(),
        links: Vec::new(),
        label: None,
        input: serde_json::Value::Null,
        output: serde_json::Value::Null,
        service_name: service.to_string(),
        resource_attributes: Vec::new(),
        service_namespace: Some("bench".to_string()),
        service_version: Some("1".to_string()),
        service_instance_id: None,
    }
}

fn make_fixture(start: usize, end: usize) -> Vec<TraceSpanRecord> {
    (start..end)
        .map(|i| {
            let service = if i % 2 == 0 { "svc_a" } else { "svc_b" };
            let planner_value = if i % 3 == 0 { "hit" } else { "miss" };
            make_span(i, service, planner_value)
        })
        .collect()
}

async fn write_fixture(service: &TraceSpanService, size: usize) {
    // The plan targets a 1M-span fixture, but building that vector at once would
    // benchmark allocator pressure instead of planner execution.
    for start in (0..size).step_by(WRITE_CHUNK_SIZE) {
        let end = (start + WRITE_CHUNK_SIZE).min(size);
        service
            .write_spans_direct(make_fixture(start, end))
            .await
            .unwrap();
    }
}

fn metrics_request(clause: FilterClause) -> TraceMetricsRequest {
    let end = Utc::now() + chrono::Duration::minutes(1);
    TraceMetricsRequest {
        start_time: end - chrono::Duration::hours(1),
        end_time: end,
        bucket_interval: "minute".to_string(),
        clause: Some(clause),
        entity_uid: None,
        query: None,
    }
}

fn benchmark_planner_queries(c: &mut Criterion) {
    if !tiers::tier_guard_for("planner_bench", "planner_queries") {
        return;
    }

    let mut group = c.benchmark_group("planner_queries");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let rt = Runtime::new().unwrap();
    let tmp_dir = tempfile::tempdir().unwrap();
    let settings = storage_settings(tmp_dir.path().to_string_lossy().to_string());
    let service = rt.block_on(async {
        let service = TraceSpanService::new(&settings, 999, Some(1), None, 10, None)
            .await
            .unwrap();
        write_fixture(&service, PLANNER_BENCH_SPANS).await;
        service
    });
    let service = Arc::new(service);

    let cases = vec![
        (
            "service_and_phrase",
            FilterClause::And(vec![
                FilterClause::Service("svc_a".to_string()),
                FilterClause::Attr {
                    key: "planner".to_string(),
                    value: "hit".to_string(),
                },
            ]),
        ),
        (
            "not_phrase",
            FilterClause::Not(Box::new(FilterClause::Attr {
                key: "planner".to_string(),
                value: "miss".to_string(),
            })),
        ),
        (
            "mixed_or",
            FilterClause::Or(vec![
                FilterClause::Service("svc_b".to_string()),
                FilterClause::Attr {
                    key: "planner".to_string(),
                    value: "hit".to_string(),
                },
            ]),
        ),
    ];

    for (name, clause) in cases {
        let request = Arc::new(metrics_request(clause));
        let service = service.clone();
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.to_async(&rt).iter_custom(|iters| {
                let svc = service.clone();
                let request = request.clone();
                async move {
                    let start = Instant::now();
                    for _ in 0..iters {
                        let queries = TraceQueries::new(svc.ctx.clone());
                        let _ = black_box(
                            queries
                                .get_trace_metrics(&request, &request.bucket_interval)
                                .await
                                .unwrap(),
                        );
                    }
                    start.elapsed()
                }
            });
        });
    }

    group.finish();
    let service =
        Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
    rt.block_on(async { service.shutdown().await.unwrap() });
    drop(tmp_dir);
}

criterion_group!(benches, benchmark_planner_queries);
criterion_main!(benches);
