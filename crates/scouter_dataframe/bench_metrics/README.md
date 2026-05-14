# Tier 0 Benchmark Baselines

This directory contains the committed Tier 0 artifacts produced by `make bench.core`.

Tier 0 is the blocking OLAP smoke baseline. It is intentionally small enough for PR verification,
but it must still prove the measured path ran:

- every registered Tier 0 group must write an artifact;
- non-sentinel groups must report measured `bench.query.end_to_end` iterations, not a single
  setup probe;
- non-sentinel groups must report `query_entrypoint` and `result_rows`;
- non-sentinel groups must report object-store operations observed through the production
  object-store spans;
- `t0_refresh_origin_sentinel` is allowed to report zero workload metrics, because it only guards
  that refresh-on-request accounting stays at zero.

## End-to-end measurement boundary

`bench.query.end_to_end` is the primary Tier 0 metric. It wraps the public in-process query
entry point and includes the returned batches or domain objects, so future phases can catch
regressions in planning, metadata lookup, snapshot freshness, DataFusion execution, and result
assembly.

Every non-sentinel Tier 0 artifact must include:

- `query_entrypoint`: the in-process boundary being measured.
- `result_rows`: the number of returned rows or spans from the probe query.
- `spans["bench.query.end_to_end"]`: at least 10 measured iterations with non-zero total time.

`df.collect` is diagnostic only. An improvement in `df.collect` cannot mask a
`bench.query.end_to_end.p95_us` regression, though diagnostic regressions still fail Tier 0 when
both baseline and run artifacts carry the span.

`t0_refresh_origin_sentinel` does not execute a query, so it may omit
`bench.query.end_to_end`, `query_entrypoint`, and `result_rows`.

Baseline JSON files are refreshed by an explicit reviewer-visible PR after a corrected Tier 0
Criterion run. CI compares artifacts but never writes committed baselines.

The comparator hard-fails only Tier 0 artifacts. Tier 1 and Tier 2 artifacts are advisory and are
intended for extended or release certification runs.
