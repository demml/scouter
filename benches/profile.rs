use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::Array;
use ndarray_rand::rand_distr::Uniform;
use ndarray_rand::RandomExt;
use scouter::math::stats::Monitor;

fn criterion_benchmark(c: &mut Criterion) {
    let monitor = Monitor::new();
    let mut group = c.benchmark_group("sample-size-example");
    let array = Array::random((10000, 10), Uniform::new(0., 10.));
    let features: Vec<String> = (0..200).map(|x| x.to_string()).collect();
    group.bench_function("fib 20", |b| {
        b.iter(|| monitor.create_2d_monitor_profile(&features, black_box(array.view())))
    });
    group.sample_size(10);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
