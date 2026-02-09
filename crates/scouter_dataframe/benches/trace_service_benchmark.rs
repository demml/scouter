use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_mocks::create_simple_trace;
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::TraceSpan;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn generate_trace_batch(num_traces: usize, spans_per_trace: usize) -> Vec<TraceSpan> {
    (0..num_traces)
        .flat_map(|_| {
            let mut trace = create_simple_trace();
            // Duplicate trace pattern to create more spans
            let base_spans = trace.clone();
            for _ in 1..spans_per_trace / 3 {
                trace.extend(base_spans.clone());
            }
            trace
        })
        .collect()
}

fn bench_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_throughput");

    // Test different batch sizes
    for batch_size in [100, 1_000, 10_000, 50_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let storage_settings = ObjectStorageSettings::default();
                    let service = TraceSpanService::new(&storage_settings, 24, Some(60))
                        .await
                        .unwrap();

                    let spans = generate_trace_batch(size / 3, 3);

                    service.write_spans(black_box(spans)).await.unwrap();

                    // Wait for buffer flush
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    service.shutdown().await.unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_concurrent_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_writes");

    for num_writers in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_writers),
            num_writers,
            |b, &writers| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let storage_settings = ObjectStorageSettings::default();
                    let service = Arc::new(
                        TraceSpanService::new(&storage_settings, 24, Some(5))
                            .await
                            .unwrap(),
                    );

                    let mut handles = vec![];

                    for _ in 0..writers {
                        let service_clone = service.clone();
                        let handle = tokio::spawn(async move {
                            for _ in 0..100 {
                                let spans = create_simple_trace();
                                service_clone.write_spans(spans).await.unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }

                    tokio::time::sleep(Duration::from_secs(2)).await;
                    drop(service);
                });
            },
        );
    }
    group.finish();
}

fn bench_query_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_performance");

    for dataset_size in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(dataset_size),
            dataset_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();

                // Setup: Pre-populate with data
                let (service, trace_id) = rt.block_on(async {
                    let storage_settings = ObjectStorageSettings::default();
                    let service = TraceSpanService::new(&storage_settings, 24, Some(60))
                        .await
                        .unwrap();

                    let spans = generate_trace_batch(size / 3, 3);
                    let trace_id = spans[0].trace_id.clone();

                    service.write_spans(spans).await.unwrap();
                    tokio::time::sleep(Duration::from_secs(3)).await;

                    (service, trace_id)
                });

                b.to_async(&rt).iter(|| async {
                    use scouter_types::TraceId;
                    let trace_id_bytes = TraceId::hex_to_bytes(&trace_id).unwrap();

                    let results = service
                        .query_service
                        .get_trace_spans(Some(&trace_id_bytes), None, None, None, None)
                        .await
                        .unwrap();

                    black_box(results);
                });

                rt.block_on(async {
                    service.shutdown().await.unwrap();
                });
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

        b.to_async(&rt).iter(|| async {
            let storage_settings = ObjectStorageSettings::default();
            let service = TraceSpanService::new(&storage_settings, 24, Some(5))
                .await
                .unwrap();

            // Simulate 20M spans/day = ~231 spans/second
            // Run for 10 seconds = 2,310 spans
            let spans_per_batch = 100;
            let num_batches = 23;

            for _ in 0..num_batches {
                let spans = generate_trace_batch(spans_per_batch / 3, 3);
                service.write_spans(spans).await.unwrap();
                tokio::time::sleep(Duration::from_millis(434)).await; // ~230 spans/sec
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
            service.shutdown().await.unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_write_throughput,
    bench_concurrent_writes,
    bench_query_performance,
    bench_sustained_load
);
criterion_main!(benches);
