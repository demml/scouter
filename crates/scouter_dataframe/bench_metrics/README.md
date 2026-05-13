Tier 0 baseline JSON artifacts live in this directory after `make bench.core`
has been proven to complete under the 15 minute Phase 0.6 budget.

Benchmark runs write fresh artifacts to `target/bench_metrics/`; CI must not
write into this directory.
