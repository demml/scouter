use chrono::{DateTime, Utc};
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

const TOTAL_SPANS: usize = 1_000_000;
const SPANS_PER_TRACE: usize = 5;
const WRITE_CHUNK_SPANS: usize = 50_000;
const SERVICE_COUNT: usize = 20;
const HOT_SERVICE: &str = "service_03";

#[derive(Clone)]
struct HotFixture {
    service: Arc<TraceSpanService>,
    trace_id: Arc<Vec<u8>>,
    trace_start: DateTime<Utc>,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
}

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

fn trace_id(trace_idx: usize) -> TraceId {
    TraceId::from_bytes((trace_idx as u128).to_be_bytes())
}

fn span_id(trace_idx: usize, span_idx: usize) -> SpanId {
    SpanId::from_bytes(((trace_idx * SPANS_PER_TRACE + span_idx) as u64).to_be_bytes())
}

fn span_record(trace_idx: usize, span_idx: usize, base_time: DateTime<Utc>) -> TraceSpanRecord {
    let hour = trace_idx % 24;
    let within_hour_ms = ((trace_idx / 24) % 3_600_000) as i64;
    let start = base_time
        + chrono::Duration::hours(hour as i64)
        + chrono::Duration::milliseconds(within_hour_ms)
        + chrono::Duration::milliseconds(span_idx as i64);
    let duration_ms = 10 + (span_idx as i64 * 5);
    let service_name = format!("service_{:02}", trace_idx % SERVICE_COUNT);
    let status_code = if trace_idx.is_multiple_of(17) { 2 } else { 0 };
    let root_span_id = span_id(trace_idx, 0);

    TraceSpanRecord {
        created_at: start,
        trace_id: trace_id(trace_idx),
        span_id: span_id(trace_idx, span_idx),
        parent_span_id: (span_idx != 0).then_some(root_span_id),
        flags: 1,
        trace_state: String::new(),
        scope_name: "hot_path.bench".to_string(),
        scope_version: Some("1".to_string()),
        span_name: if span_idx == 0 {
            "http.request".to_string()
        } else {
            format!("child_op_{span_idx}")
        },
        span_kind: if span_idx == 0 {
            "SERVER".to_string()
        } else {
            "INTERNAL".to_string()
        },
        start_time: start,
        end_time: start + chrono::Duration::milliseconds(duration_ms),
        duration_ms,
        status_code,
        status_message: if status_code == 2 {
            "Internal Server Error".to_string()
        } else {
            "OK".to_string()
        },
        attributes: vec![
            Attribute {
                key: "component".to_string(),
                value: json!(if trace_idx.is_multiple_of(11) {
                    "kafka"
                } else {
                    "http"
                }),
            },
            Attribute {
                key: "tenant".to_string(),
                value: json!(format!("tenant_{}", trace_idx % 50)),
            },
        ],
        events: Vec::new(),
        links: Vec::new(),
        label: None,
        input: serde_json::Value::Null,
        output: serde_json::Value::Null,
        service_name,
        resource_attributes: Vec::new(),
        service_namespace: Some("bench".to_string()),
        service_version: Some("1".to_string()),
        service_instance_id: Some(format!("instance_{}", trace_idx % 8)),
    }
}

fn span_chunk(
    start_span: usize,
    end_span: usize,
    base_time: DateTime<Utc>,
) -> Vec<TraceSpanRecord> {
    (start_span..end_span)
        .map(|span_global_idx| {
            let trace_idx = span_global_idx / SPANS_PER_TRACE;
            let span_idx = span_global_idx % SPANS_PER_TRACE;
            span_record(trace_idx, span_idx, base_time)
        })
        .collect()
}

async fn seed_fixture(settings: &ObjectStorageSettings) -> HotFixture {
    let service = TraceSpanService::new(settings, 999, Some(1), None, 10)
        .await
        .unwrap();
    let base_time = Utc::now() - chrono::Duration::hours(24);

    // Direct chunked writes keep setup memory bounded and avoid measuring the
    // buffering actor when the benchmark is about read-path latency.
    for start in (0..TOTAL_SPANS).step_by(WRITE_CHUNK_SPANS) {
        let end = (start + WRITE_CHUNK_SPANS).min(TOTAL_SPANS);
        service
            .write_spans_direct(span_chunk(start, end, base_time))
            .await
            .unwrap();
    }

    // Hot-path expectations assume optimized files: Z-ORDER on start_time and
    // service_name plus compacted file sizes close to production layout.
    service.optimize().await.unwrap();

    let trace_idx = 24 * 1_000 + 3;
    let trace_start = base_time
        + chrono::Duration::hours((trace_idx % 24) as i64)
        + chrono::Duration::milliseconds(((trace_idx / 24) % 3_600_000) as i64);

    HotFixture {
        service: Arc::new(service),
        trace_id: Arc::new(trace_id(trace_idx).as_bytes().to_vec()),
        trace_start,
        window_start: base_time + chrono::Duration::hours(3),
        window_end: base_time + chrono::Duration::hours(4),
    }
}

fn metrics_request(
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    clause: Option<FilterClause>,
) -> TraceMetricsRequest {
    TraceMetricsRequest {
        start_time,
        end_time,
        bucket_interval: "minute".to_string(),
        clause,
        entity_uid: None,
        query: None,
    }
}

fn benchmark_trace_hot_paths(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let tmp_dir = tempfile::tempdir().unwrap();
    let settings = storage_settings(tmp_dir.path().to_string_lossy().to_string());
    let fixture = rt.block_on(seed_fixture(&settings));

    let mut group = c.benchmark_group("trace_hot_paths_1m");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("trace_id_uncached_no_time", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let fixture = fixture.clone();
            async move {
                let start = Instant::now();
                for _ in 0..iters {
                    let _ = black_box(
                        fixture
                            .service
                            .query_service
                            .query_spans(
                                Some(&fixture.trace_id),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            )
                            .await
                            .unwrap(),
                    );
                }
                start.elapsed()
            }
        });
    });

    group.bench_function("trace_id_uncached_1h", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let fixture = fixture.clone();
            async move {
                let start_time = fixture.trace_start - chrono::Duration::minutes(1);
                let end_time = fixture.trace_start + chrono::Duration::minutes(1);
                let start = Instant::now();
                for _ in 0..iters {
                    let _ = black_box(
                        fixture
                            .service
                            .query_service
                            .query_spans(
                                Some(&fixture.trace_id),
                                None,
                                None,
                                None,
                                None,
                                Some(&start_time),
                                Some(&end_time),
                                None,
                            )
                            .await
                            .unwrap(),
                    );
                }
                start.elapsed()
            }
        });
    });

    rt.block_on(async {
        let _ = fixture
            .service
            .query_service
            .get_trace_spans(
                Some(&fixture.trace_id),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
    });
    group.bench_function("trace_id_lru_cached", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let fixture = fixture.clone();
            async move {
                let start = Instant::now();
                for _ in 0..iters {
                    let _ = black_box(
                        fixture
                            .service
                            .query_service
                            .get_trace_spans(
                                Some(&fixture.trace_id),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            )
                            .await
                            .unwrap(),
                    );
                }
                start.elapsed()
            }
        });
    });

    group.bench_function("service_name_1h_limit_1000", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let fixture = fixture.clone();
            async move {
                let start = Instant::now();
                for _ in 0..iters {
                    let _ = black_box(
                        fixture
                            .service
                            .query_service
                            .query_spans(
                                None,
                                Some(HOT_SERVICE),
                                None,
                                None,
                                None,
                                Some(&fixture.window_start),
                                Some(&fixture.window_end),
                                Some(1_000),
                            )
                            .await
                            .unwrap(),
                    );
                }
                start.elapsed()
            }
        });
    });

    group.bench_function("service_tuple_1h_limit_1000", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let fixture = fixture.clone();
            async move {
                let start = Instant::now();
                for _ in 0..iters {
                    let _ = black_box(
                        fixture
                            .service
                            .query_service
                            .query_spans(
                                None,
                                Some(HOT_SERVICE),
                                Some("bench"),
                                Some("1"),
                                Some("instance_3"),
                                Some(&fixture.window_start),
                                Some(&fixture.window_end),
                                Some(1_000),
                            )
                            .await
                            .unwrap(),
                    );
                }
                start.elapsed()
            }
        });
    });

    group.finish();

    let mut metrics_group = c.benchmark_group("metrics_hot_paths_1m");
    metrics_group.sample_size(10);
    metrics_group.measurement_time(Duration::from_secs(5));

    let cases = [
        (
            "service_only_1h",
            Some(FilterClause::Service(HOT_SERVICE.to_string())),
        ),
        ("status_error_1h", Some(FilterClause::StatusCode(2))),
        (
            "duration_range_1h",
            Some(FilterClause::And(vec![
                FilterClause::DurationMinMs(10),
                FilterClause::DurationMaxMs(30),
            ])),
        ),
        (
            "attr_only_1h",
            Some(FilterClause::Attr {
                key: "component".to_string(),
                value: "kafka".to_string(),
            }),
        ),
        (
            "service_and_attr_1h",
            Some(FilterClause::And(vec![
                FilterClause::Service(HOT_SERVICE.to_string()),
                FilterClause::Attr {
                    key: "component".to_string(),
                    value: "kafka".to_string(),
                },
            ])),
        ),
    ];

    for (name, clause) in cases {
        let request = Arc::new(metrics_request(
            fixture.window_start,
            fixture.window_end,
            clause,
        ));
        metrics_group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.to_async(&rt).iter_custom(|iters| {
                let fixture = fixture.clone();
                let request = request.clone();
                async move {
                    let start = Instant::now();
                    for _ in 0..iters {
                        let queries = TraceQueries::new(fixture.service.ctx.clone());
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

    metrics_group.finish();

    let service = Arc::try_unwrap(fixture.service)
        .unwrap_or_else(|_| panic!("Arc still has multiple owners"));
    rt.block_on(async { service.shutdown().await.unwrap() });
    drop(tmp_dir);
}

criterion_group!(benches, benchmark_trace_hot_paths);
criterion_main!(benches);
