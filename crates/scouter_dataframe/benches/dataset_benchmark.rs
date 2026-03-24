use arrow::array::{Date32Array, Float64Array, StringArray, TimestampMicrosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::RecordBatch;
use chrono::{Datelike, Utc};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use scouter_dataframe::parquet::bifrost::manager::DatasetEngineManager;
use scouter_settings::ObjectStorageSettings;
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
use scouter_types::StorageType;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

fn bench_schema() -> Schema {
    Schema::new(vec![
        Field::new("user_id", DataType::Utf8, false),
        Field::new("score", DataType::Float64, false),
        Field::new("model_name", DataType::Utf8, true),
        Field::new(
            "scouter_created_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new("scouter_partition_date", DataType::Date32, false),
        Field::new("scouter_batch_id", DataType::Utf8, false),
    ])
}

fn make_batch(schema: &Schema, n: usize) -> RecordBatch {
    let now = Utc::now();
    let epoch_days = now.date_naive().num_days_from_ce() - 719_163;

    RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(StringArray::from_iter_values(
                (0..n).map(|i| format!("u{i}")),
            )),
            Arc::new(Float64Array::from_iter_values(
                (0..n).map(|i| 0.5 + (i % 50) as f64 / 100.0),
            )),
            Arc::new(StringArray::from_iter(
                (0..n).map(|i| Some(if i % 2 == 0 { "model_a" } else { "model_b" })),
            )),
            Arc::new(
                TimestampMicrosecondArray::from_iter_values((0..n).map(|_| now.timestamp_micros()))
                    .with_timezone("UTC"),
            ),
            Arc::new(Date32Array::from_iter_values((0..n).map(|_| epoch_days))),
            Arc::new(StringArray::from_iter_values((0..n).map(|_| "bench-batch"))),
        ],
    )
    .unwrap()
}

fn make_registration(schema: &Schema) -> DatasetRegistration {
    let arrow_json = serde_json::to_string(schema).unwrap();
    let fingerprint = DatasetFingerprint::from_schema_json(&arrow_json);
    let namespace = DatasetNamespace::new("bench", "ds", "preds").unwrap();
    DatasetRegistration::new(namespace, fingerprint, arrow_json, "{}".into(), vec![])
}

fn make_storage_settings(dir: &tempfile::TempDir) -> ObjectStorageSettings {
    ObjectStorageSettings {
        storage_uri: dir.path().to_str().unwrap().to_string(),
        storage_type: StorageType::Local,
        region: "us-east-1".to_string(),
        trace_compaction_interval_hours: 999,
        trace_flush_interval_secs: 1,
        trace_refresh_interval_secs: 10,
    }
}

fn bench_write_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("dataset_write");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for batch_size in [100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                let schema = bench_schema();

                b.to_async(&rt).iter(|| async {
                    let dir = tempfile::tempdir().unwrap();
                    let settings = make_storage_settings(&dir);
                    let manager =
                        DatasetEngineManager::with_config(&settings, 1800, 10, 1, 50_000, 30)
                            .await
                            .unwrap();

                    let reg = make_registration(&schema);
                    manager.register_dataset(&reg).await.unwrap();

                    let batch = make_batch(&schema, size);
                    manager
                        .insert_batch(&reg.namespace, &reg.fingerprint, black_box(batch))
                        .await
                        .unwrap();

                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    manager.shutdown().await;
                });
            },
        );
    }
    group.finish();
}

fn bench_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let schema = bench_schema();

    // Seed 10K rows once outside the timed loop
    let (manager, namespace) = rt.block_on(async {
        let settings = make_storage_settings(&dir);
        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 200_000, 30)
            .await
            .unwrap();

        let reg = make_registration(&schema);
        manager.register_dataset(&reg).await.unwrap();

        for _ in 0..100 {
            let batch = make_batch(&schema, 100);
            manager
                .insert_batch(&reg.namespace, &reg.fingerprint, batch)
                .await
                .unwrap();
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
        let ns = reg.namespace.clone();
        (manager, ns)
    });

    let manager = Arc::new(manager);
    let fqn = namespace.fqn();

    let mut group = c.benchmark_group("dataset_query");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("count_star", |b| {
        let mgr = Arc::clone(&manager);
        let sql = format!("SELECT COUNT(*) as cnt FROM {fqn}");
        b.to_async(&rt)
            .iter(|| async { black_box(mgr.query(&sql).await.unwrap()) });
    });

    group.bench_function("select_star", |b| {
        let mgr = Arc::clone(&manager);
        let sql = format!("SELECT * FROM {fqn} LIMIT 1000");
        b.to_async(&rt)
            .iter(|| async { black_box(mgr.query(&sql).await.unwrap()) });
    });

    group.bench_function("filter_model_name", |b| {
        let mgr = Arc::clone(&manager);
        let sql = format!("SELECT * FROM {fqn} WHERE model_name = 'model_a'");
        b.to_async(&rt)
            .iter(|| async { black_box(mgr.query(&sql).await.unwrap()) });
    });

    group.finish();

    rt.block_on(async {
        Arc::try_unwrap(manager)
            .unwrap_or_else(|_| panic!("manager still referenced"))
            .shutdown()
            .await;
    });
}

criterion_group!(benches, bench_write_throughput, bench_query);
criterion_main!(benches);
