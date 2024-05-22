use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::Array;
use ndarray_rand::rand_distr::Uniform;
use ndarray_rand::RandomExt;
use scouter::math::monitor::Monitor;

use scouter::math::profiler::Profiler;

fn criterion_benchmark(c: &mut Criterion) {
    let monitor = Monitor::new();
    let profiler = Profiler::default();
    let mut group = c.benchmark_group("sample-size-example");
    let array = Array::random((1000, 10), Uniform::new(0., 10.));
    let features: Vec<String> = (0..10).map(|x| x.to_string()).collect();
    group.bench_function("monitor", |b| {
        b.iter(|| monitor.create_2d_monitor_profile(&features, black_box(&array.view())))
    });
    group.bench_function("profile", |b| {
        b.iter(|| profiler.compute_stats(&features, black_box(&array.view())))
    });
    group.sample_size(10);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
