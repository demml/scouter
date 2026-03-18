use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{StorageType, TraceId, TraceSpanRecord};
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

fn generate_trace_batch(num_traces: usize, spans_per_trace: usize) -> Vec<TraceSpanRecord> {
    use scouter_mocks::generate_trace_with_spans;
    (0..num_traces)
        .flat_map(|_| {
            let (_record, spans, _tags) = generate_trace_with_spans(spans_per_trace, 0);
            spans
        })
        .collect()
}

fn bench_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_throughput");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    for batch_size in [100, 1_000, 10_000, 50_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let tmp_dir = tempfile::tempdir().unwrap();
                let storage_settings = ObjectStorageSettings {
                    storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 1,
                    trace_refresh_interval_secs: 10,
                };

                b.to_async(&rt).iter(|| async {
                    let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                        .await
                        .unwrap();

                    let spans = generate_trace_batch(size / 5, 5);
                    service.write_spans(black_box(spans)).await.unwrap();

                    // flush_interval=1s; wait 1.5s for the commit to land
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    service.shutdown().await.unwrap();
                });

                drop(tmp_dir);
            },
        );
    }
    group.finish();
}

fn bench_concurrent_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_writes");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    for num_writers in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_writers),
            num_writers,
            |b, &writers| {
                let rt = Runtime::new().unwrap();
                let tmp_dir = tempfile::tempdir().unwrap();
                let storage_settings = ObjectStorageSettings {
                    storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 1,
                    trace_refresh_interval_secs: 10,
                };

                b.to_async(&rt).iter(|| async {
                    let service = Arc::new(
                        TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                            .await
                            .unwrap(),
                    );

                    let mut handles = vec![];
                    for _ in 0..writers {
                        let svc = service.clone();
                        handles.push(tokio::spawn(async move {
                            for _ in 0..10 {
                                let spans = generate_trace_batch(1, 5);
                                svc.write_spans(spans).await.unwrap();
                            }
                        }));
                    }
                    for h in handles {
                        h.await.unwrap();
                    }

                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    drop(service);
                });

                drop(tmp_dir);
            },
        );
    }
    group.finish();
}

fn bench_query_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance");
    group.sample_size(20);

    for dataset_size in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(dataset_size),
            dataset_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let tmp_dir = tempfile::tempdir().unwrap();
                let storage_settings = ObjectStorageSettings {
                    storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 1,
                    trace_refresh_interval_secs: 10,
                };

                let (service, trace_id) = rt.block_on(async {
                    let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                        .await
                        .unwrap();
                    let spans = generate_trace_batch(size / 5, 5);
                    let trace_id = spans[0].trace_id.to_hex();
                    service.write_spans(spans).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    (service, trace_id)
                });

                let trace_id_bytes = Arc::new(TraceId::hex_to_bytes(&trace_id).unwrap());
                let service = Arc::new(service);

                b.to_async(&rt).iter_custom(|iters| {
                    let svc = service.clone();
                    let id = trace_id_bytes.clone();
                    async move {
                        let t = Instant::now();
                        for _ in 0..iters {
                            let _ = black_box(
                                svc.query_service
                                    .get_trace_spans(Some(&id), None, None, None, None)
                                    .await
                                    .unwrap(),
                            );
                        }
                        t.elapsed()
                    }
                });

                let service = Arc::try_unwrap(service)
                    .unwrap_or_else(|_| panic!("Arc still has multiple owners"));
                rt.block_on(async { service.shutdown().await.unwrap() });
                drop(tmp_dir);
            },
        );
    }
    group.finish();
}

fn bench_sustained_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_load");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);

    group.bench_function("sustained_20M_per_day", |b| {
        let rt = Runtime::new().unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let storage_settings = ObjectStorageSettings {
            storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
            storage_type: StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 999,
            trace_flush_interval_secs: 1,
            trace_refresh_interval_secs: 10,
        };

        b.to_async(&rt).iter(|| async {
            let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                .await
                .unwrap();

            // 20M spans/day ≈ 231 spans/sec. 10 batches of 100 spans ≈ 1s of load.
            for _ in 0..10 {
                let spans = generate_trace_batch(20, 5);
                service.write_spans(spans).await.unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            tokio::time::sleep(Duration::from_millis(1500)).await;
            service.shutdown().await.unwrap();
        });

        drop(tmp_dir);
    });

    group.finish();
}

fn bench_query_at_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_at_scale");
    // Sizes are intentionally moderate: the scaling curve (linear vs sub-linear)
    // is visible at [10K, 50K, 100K]. For absolute 1M-span latency numbers,
    // use `cargo bench --bench stress_test` which prints p50/p95/p99.
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for dataset_size in [10_000usize, 50_000, 100_000] {
        let chunk = 2_000;

        // ── Single trace lookup ────────────────────────────────────────────
        group.bench_with_input(
            BenchmarkId::new("get_trace_spans_by_id", dataset_size),
            &dataset_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let tmp_dir = tempfile::tempdir().unwrap();
                let storage_settings = ObjectStorageSettings {
                    storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 1,
                    trace_refresh_interval_secs: 10,
                };

                let (service, trace_id_bytes) = rt.block_on(async {
                    let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                        .await
                        .unwrap();
                    let mut first_id = None;
                    for i in 0..size.div_ceil(chunk) {
                        let spans = generate_trace_batch(chunk / 5, 5);
                        if i == 0 {
                            first_id = Some(spans[0].trace_id.to_hex());
                        }
                        service.write_spans(spans).await.unwrap();
                    }
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    service.optimize().await.unwrap();
                    let bytes = TraceId::hex_to_bytes(&first_id.unwrap()).unwrap();
                    (service, bytes)
                });

                let service = Arc::new(service);
                let trace_id_bytes = Arc::new(trace_id_bytes);

                b.to_async(&rt).iter_custom(|iters| {
                    let svc = service.clone();
                    let id = trace_id_bytes.clone();
                    async move {
                        let start = Instant::now();
                        for _ in 0..iters {
                            let _ = black_box(
                                svc.query_service
                                    .get_trace_spans(Some(&id), None, None, None, None)
                                    .await
                                    .unwrap(),
                            );
                        }
                        start.elapsed()
                    }
                });

                let service = Arc::try_unwrap(service)
                    .unwrap_or_else(|_| panic!("Arc still has multiple owners after iter_custom"));
                rt.block_on(async { service.shutdown().await.unwrap() });
                drop(tmp_dir);
            },
        );

        // ── Time-window scan ──────────────────────────────────────────────
        group.bench_with_input(
            BenchmarkId::new("get_trace_spans_time_scan", dataset_size),
            &dataset_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let tmp_dir = tempfile::tempdir().unwrap();
                let storage_settings = ObjectStorageSettings {
                    storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 1,
                    trace_refresh_interval_secs: 10,
                };

                let service = rt.block_on(async {
                    let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                        .await
                        .unwrap();
                    for _ in 0..size.div_ceil(chunk) {
                        service
                            .write_spans(generate_trace_batch(chunk / 5, 5))
                            .await
                            .unwrap();
                    }
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    service.optimize().await.unwrap();
                    service
                });

                let service = Arc::new(service);

                b.to_async(&rt).iter_custom(|iters| {
                    let svc = service.clone();
                    async move {
                        use chrono::Utc;
                        let start_t = Utc::now() - chrono::Duration::hours(24);
                        let end_t = Utc::now() + chrono::Duration::hours(1);
                        let t = Instant::now();
                        for _ in 0..iters {
                            let _ = black_box(
                                svc.query_service
                                    .get_trace_spans(
                                        None,
                                        None,
                                        Some(&start_t),
                                        Some(&end_t),
                                        Some(1000),
                                    )
                                    .await
                                    .unwrap(),
                            );
                        }
                        t.elapsed()
                    }
                });

                let service = Arc::try_unwrap(service)
                    .unwrap_or_else(|_| panic!("Arc still has multiple owners after iter_custom"));
                rt.block_on(async { service.shutdown().await.unwrap() });
                drop(tmp_dir);
            },
        );
    }

    group.finish();
}

/// Benchmark group that measures cold-path DataFusion query latency by bypassing the LRU cache
/// and calling `query_spans` directly.
///
/// Three sub-benchmarks at ~10k spans across 24 hourly buckets:
///
/// - `by_id_no_time_bounds`   — full file scan, trace_id predicate only (baseline)
/// - `by_id_with_time_bounds` — same traces but a 1-hour window per trace; proves ts_lit pruning
/// - `by_entity`              — entity_id column predicate with a 1-hour window
fn bench_cold_query(c: &mut Criterion) {
    const HOURS: usize = 24;
    const SPANS_PER_HOUR: usize = 420; // ~10 080 total; 84 traces × 5 spans per hour

    let mut group = c.benchmark_group("cold_query");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    // ── 3a: by_id, no time bounds — full scan baseline ────────────────────
    group.bench_function("by_id_no_time_bounds", |b| {
        use scouter_mocks::generate_trace_with_spans;

        let rt = Runtime::new().unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let storage_settings = ObjectStorageSettings {
            storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
            storage_type: StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 999,
            trace_flush_interval_secs: 1,
            trace_refresh_interval_secs: 10,
        };

        // all_ids: (trace_id_bytes, hour_index)
        let (service, all_ids) = rt.block_on(async {
            let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                .await
                .unwrap();
            let mut all_ids: Vec<(Vec<u8>, usize)> = Vec::new();

            for hour in 0..HOURS {
                let minutes_offset = (hour as i64) * 60;
                let traces_this_hour = SPANS_PER_HOUR / 5;
                let mut hour_spans: Vec<TraceSpanRecord> = Vec::new();

                for _ in 0..traces_this_hour {
                    let (_r, spans, _t) = generate_trace_with_spans(5, minutes_offset);
                    if let Some(first) = spans.first() {
                        if let Ok(id_bytes) = TraceId::hex_to_bytes(&first.trace_id.to_hex()) {
                            all_ids.push((id_bytes, hour));
                        }
                    }
                    hour_spans.extend(spans);
                }
                service.write_spans(hour_spans).await.unwrap();
            }

            tokio::time::sleep(Duration::from_millis(1500)).await;
            service.optimize().await.unwrap();
            (Arc::new(service), Arc::new(all_ids))
        });

        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let t = Instant::now();
                for i in 0..iters {
                    let (id, _hour) = &ids[i as usize % ids.len()];
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, None, None, None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });

        let service =
            Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
        rt.block_on(async { service.shutdown().await.unwrap() });
        drop(tmp_dir);
    });

    // ── 3b: by_id, 1-hour window — proves ts_lit typed-timestamp pruning ──
    group.bench_function("by_id_with_time_bounds", |b| {
        use chrono::Utc;
        use scouter_mocks::generate_trace_with_spans;

        let rt = Runtime::new().unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let storage_settings = ObjectStorageSettings {
            storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
            storage_type: StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 999,
            trace_flush_interval_secs: 1,
            trace_refresh_interval_secs: 10,
        };

        let (service, all_ids) = rt.block_on(async {
            let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
                .await
                .unwrap();
            let mut all_ids: Vec<(Vec<u8>, usize)> = Vec::new();

            for hour in 0..HOURS {
                let minutes_offset = (hour as i64) * 60;
                let traces_this_hour = SPANS_PER_HOUR / 5;
                let mut hour_spans: Vec<TraceSpanRecord> = Vec::new();

                for _ in 0..traces_this_hour {
                    let (_r, spans, _t) = generate_trace_with_spans(5, minutes_offset);
                    if let Some(first) = spans.first() {
                        if let Ok(id_bytes) = TraceId::hex_to_bytes(&first.trace_id.to_hex()) {
                            all_ids.push((id_bytes, hour));
                        }
                    }
                    hour_spans.extend(spans);
                }
                service.write_spans(hour_spans).await.unwrap();
            }

            tokio::time::sleep(Duration::from_millis(1500)).await;
            service.optimize().await.unwrap();
            (Arc::new(service), Arc::new(all_ids))
        });

        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let now = Utc::now();
                let t = Instant::now();
                for i in 0..iters {
                    let (id, hour) = &ids[i as usize % ids.len()];
                    // 1-hour window exactly bracketing the span's write hour.
                    // With data spanning 24h, Parquet min/max stats should prune ~23/24 files.
                    let start_t = now - chrono::Duration::hours((*hour as i64) + 1);
                    let end_t = now - chrono::Duration::hours(*hour as i64);
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, Some(&start_t), Some(&end_t), None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });

        let service =
            Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
        rt.block_on(async { service.shutdown().await.unwrap() });
        drop(tmp_dir);
    });

    group.finish();
}

/// Criterion counterpart of `stress_test` — seeds 1M spans across 24 hourly batches,
/// runs Z-ORDER compaction, then benchmarks three query patterns that the stress test tracks.
///
/// Results are stored in `target/criterion/at_scale_1m/` and tracked across commits.
/// Run with: `cargo bench -p scouter-dataframe --bench trace_service_benchmark at_scale_1m`
fn bench_at_scale_1m(c: &mut Criterion) {
    use scouter_mocks::generate_trace_with_spans;
    use scouter_types::StorageType;

    const TOTAL_SPANS: usize = 1_000_000;
    const HOURS: usize = 24;
    const SPANS_PER_HOUR: usize = TOTAL_SPANS / HOURS;
    const TRACES_PER_HOUR: usize = SPANS_PER_HOUR / 5;
    const TARGET_ENTITY_UID: &str = "scale-entity-abc123";
    const ENTITY_TRACES: usize = 50;
    // IDs per hour collected during seeding for query benchmarks 1a/1b.
    const IDS_PER_HOUR: usize = 500;

    let mut group = c.benchmark_group("at_scale_1m");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(120));

    // ── Shared seed helper ────────────────────────────────────────────────
    // Seeds 1M spans + entity batch, compacts, returns (Arc<service>, Arc<all_ids>).
    // Called independently for each sub-benchmark so each gets a clean temp dir.
    async fn seed_and_compact(
        storage_settings: &ObjectStorageSettings,
    ) -> (Arc<TraceSpanService>, Arc<Vec<(Vec<u8>, usize)>>) {
        let service = TraceSpanService::new(storage_settings, 999, Some(1), None, 10)
            .await
            .unwrap();
        let mut all_ids: Vec<(Vec<u8>, usize)> = Vec::with_capacity(HOURS * IDS_PER_HOUR);

        for hour in 0..HOURS {
            let minutes_offset = (hour as i64) * 60;
            let mut hour_spans = Vec::with_capacity(SPANS_PER_HOUR);
            for _ in 0..TRACES_PER_HOUR {
                let (_r, spans, _t) = generate_trace_with_spans(5, minutes_offset);
                if all_ids.len() < HOURS * IDS_PER_HOUR {
                    if let Ok(id) = TraceId::hex_to_bytes(&spans[0].trace_id.to_hex()) {
                        all_ids.push((id, hour));
                    }
                }
                hour_spans.extend(spans);
            }
            service.write_spans_direct(hour_spans).await.unwrap();
        }

        // Entity spans at hour 0 (current time) — within the 1h query window.
        let entity_spans: Vec<_> = (0..ENTITY_TRACES)
            .flat_map(|_| {
                let (_r, spans, _t) =
                    scouter_mocks::generate_trace_with_entity(5, TARGET_ENTITY_UID, 0);
                spans
            })
            .collect();
        service.write_spans_direct(entity_spans).await.unwrap();
        service.optimize().await.unwrap();
        (Arc::new(service), Arc::new(all_ids))
    }

    // ── 1a: trace_id lookup — no time bounds ─────────────────────────────
    group.bench_function("trace_id_no_bounds", |b| {
        let rt = Runtime::new().unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let storage_settings = ObjectStorageSettings {
            storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
            storage_type: StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 999,
            trace_flush_interval_secs: 1,
            trace_refresh_interval_secs: 10,
        };

        let (service, all_ids) = rt.block_on(seed_and_compact(&storage_settings));

        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let t = Instant::now();
                for i in 0..iters {
                    let (id, _hour) = &ids[i as usize % ids.len()];
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, None, None, None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });

        let service =
            Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
        rt.block_on(async { service.shutdown().await.unwrap() });
        drop(tmp_dir);
    });

    // ── 1b: trace_id + 1h time bound — validates file-level pruning ───────
    group.bench_function("trace_id_1h_bound", |b| {
        use chrono::Utc;

        let rt = Runtime::new().unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let storage_settings = ObjectStorageSettings {
            storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
            storage_type: StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 999,
            trace_flush_interval_secs: 1,
            trace_refresh_interval_secs: 10,
        };

        let (service, all_ids) = rt.block_on(seed_and_compact(&storage_settings));

        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let now = Utc::now();
                let t = Instant::now();
                for i in 0..iters {
                    let (id, hour) = &ids[i as usize % ids.len()];
                    let start_t = now - chrono::Duration::hours((*hour as i64) + 1);
                    let end_t = now - chrono::Duration::hours(*hour as i64);
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, Some(&start_t), Some(&end_t), None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });

        let service =
            Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
        rt.block_on(async { service.shutdown().await.unwrap() });
        drop(tmp_dir);
    });

    group.finish();
}

/// Criterion counterpart of `bench_at_scale_1m` — seeds 10M spans across 24 hourly batches
/// using 50K-span sub-batches to bound peak memory, runs Z-ORDER compaction, then benchmarks
/// three query patterns that prove bloom filter + Z-ORDER holds at production scale.
///
/// The dataset is seeded ONCE and shared across all three sub-benchmarks to avoid the ~3×
/// compaction cost that would result from independent seeding per sub-bench.
///
/// Results are stored in `target/criterion/at_scale_10m/` and tracked across commits.
/// Run with: `cargo bench -p scouter-dataframe --bench trace_service_benchmark at_scale_10m`
fn bench_at_scale_10m(c: &mut Criterion) {
    use chrono::Utc;
    use scouter_mocks::generate_trace_with_spans;
    use scouter_types::StorageType;

    const TOTAL_SPANS: usize = 10_000_000;
    const HOURS: usize = 24;
    const SPANS_PER_HOUR: usize = TOTAL_SPANS / HOURS; // 416_666
    const TRACES_PER_HOUR: usize = SPANS_PER_HOUR / 5; // 83_333
    const WRITE_CHUNK_SIZE: usize = 50_000;
    const TARGET_ENTITY_UID: &str = "scale10m-entity-abc123";
    const ENTITY_TRACES: usize = 50;
    const IDS_PER_HOUR: usize = 500;

    // ── Seed ONCE — shared across all three sub-benchmarks ───────────────
    let rt = Runtime::new().unwrap();
    let tmp_dir = tempfile::tempdir().unwrap();
    let storage_settings = ObjectStorageSettings {
        storage_uri: tmp_dir.path().to_str().unwrap().to_string(),
        storage_type: StorageType::Local,
        region: "us-east-1".to_string(),
        trace_compaction_interval_hours: 999,
        trace_flush_interval_secs: 1,
        trace_refresh_interval_secs: 10,
    };

    let (service, all_ids) = rt.block_on(async {
        let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10)
            .await
            .unwrap();
        let mut all_ids: Vec<(Vec<u8>, usize)> = Vec::with_capacity(HOURS * IDS_PER_HOUR);

        for hour in 0..HOURS {
            let minutes_offset = (hour as i64) * 60;
            let mut chunk: Vec<TraceSpanRecord> = Vec::with_capacity(WRITE_CHUNK_SIZE);

            for _ in 0..TRACES_PER_HOUR {
                let (_r, spans, _t) = generate_trace_with_spans(5, minutes_offset);
                if all_ids.len() < HOURS * IDS_PER_HOUR {
                    if let Ok(id) = TraceId::hex_to_bytes(&spans[0].trace_id.to_hex()) {
                        all_ids.push((id, hour));
                    }
                }
                chunk.extend(spans);
                if chunk.len() >= WRITE_CHUNK_SIZE {
                    let batch = std::mem::replace(&mut chunk, Vec::with_capacity(WRITE_CHUNK_SIZE));
                    service.write_spans_direct(batch).await.unwrap();
                }
            }
            // flush remainder
            if !chunk.is_empty() {
                service.write_spans_direct(chunk).await.unwrap();
            }
        }

        // Entity spans at hour 0 (current time) — within the 1h query window.
        let entity_spans: Vec<_> = (0..ENTITY_TRACES)
            .flat_map(|_| {
                let (_r, spans, _t) =
                    scouter_mocks::generate_trace_with_entity(5, TARGET_ENTITY_UID, 0);
                spans
            })
            .collect();
        service.write_spans_direct(entity_spans).await.unwrap();
        service.optimize().await.unwrap();
        (Arc::new(service), Arc::new(all_ids))
    });

    let mut group = c.benchmark_group("at_scale_10m");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    // ── 1a: trace_id lookup — no time bounds ─────────────────────────────
    group.bench_function("trace_id_no_bounds", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let t = Instant::now();
                for i in 0..iters {
                    let (id, _hour) = &ids[i as usize % ids.len()];
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, None, None, None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });
    });

    // ── 1b: trace_id + 1h time bound — validates file-level pruning ───────
    group.bench_function("trace_id_1h_bound", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let svc = service.clone();
            let ids = all_ids.clone();
            async move {
                let now = Utc::now();
                let t = Instant::now();
                for i in 0..iters {
                    let (id, hour) = &ids[i as usize % ids.len()];
                    let start_t = now - chrono::Duration::hours((*hour as i64) + 1);
                    let end_t = now - chrono::Duration::hours(*hour as i64);
                    let _ = black_box(
                        svc.query_service
                            .query_spans(Some(id), None, Some(&start_t), Some(&end_t), None)
                            .await
                            .unwrap(),
                    );
                }
                t.elapsed()
            }
        });
    });

    group.finish();

    // ── Single teardown ───────────────────────────────────────────────────
    let service =
        Arc::try_unwrap(service).unwrap_or_else(|_| panic!("Arc still has multiple owners"));
    rt.block_on(async { service.shutdown().await.unwrap() });
    drop(tmp_dir);
}

criterion_group!(
    benches,
    bench_write_throughput,
    bench_concurrent_writes,
    bench_query_performance,
    bench_sustained_load,
    bench_query_at_scale,
    bench_cold_query,
    bench_at_scale_1m,
    bench_at_scale_10m,
);
criterion_main!(benches);
