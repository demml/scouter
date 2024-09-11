use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::Array;
use ndarray_rand::rand_distr::Uniform;
use ndarray_rand::RandomExt;
use scouter::core::monitor::Monitor;

use scouter::core::num_profiler::NumProfiler;
use scouter::utils::types::{AlertConfig, DriftConfig};

fn criterion_benchmark(c: &mut Criterion) {
    let monitor = Monitor::new();
    let profiler = NumProfiler::default();
    let mut group = c.benchmark_group("sample-size-example");
    let array = Array::random((1000, 10), Uniform::new(0., 10.));
    let features: Vec<String> = (0..10).map(|x| x.to_string()).collect();
    let alert_config = AlertConfig::default();
    let config = DriftConfig::new(
        Some("name".to_string()),
        Some("repo".to_string()),
        None,
        None,
        None,
        None,
        None,
        Some(alert_config),
        None,
    )
    .unwrap();
    group.bench_function("monitor", |b| {
        b.iter(|| monitor.create_2d_drift_profile(&features, black_box(&array.view()), &config))
    });
    group.bench_function("profile", |b| {
        b.iter(|| {
            profiler
                .compute_stats(&features, black_box(&array.view()), &20)
                .unwrap();
        })
    });
    group.sample_size(10);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
