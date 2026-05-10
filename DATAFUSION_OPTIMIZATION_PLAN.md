# DataFusion Query Performance Optimization Plan

**Branch:** `claude/optimize-datafusion-queries-03js8`
**Status:** Proposal — pending review
**Owner:** TBD

This document captures the diagnosis, design decisions, and staged implementation plan for reducing read-path latency on Scouter's DataFusion + GCS query engine. It supersedes ad-hoc notes about result caching, partitioning, and compaction tuning.

---

## 1. Executive Summary

Scouter's analytical reads use DataFusion over Delta Lake on GCS. End-to-end query latency is currently dominated by **object-store roundtrips on the cold path** — not by DataFusion execution. A typical cold trace lookup pays for: TLS reuse, `_delta_log` read, Parquet footer fetch, bloom filter pages, then row-group data. Each round-trip is ~30–80 ms from a Docker container to GCS in the same region.

The existing setup is already well-tuned at the per-file level (Z-ORDER, ZSTD compression, DELTA_BINARY_PACKED encoding, bloom filters on selective columns, RAM range cache for Parquet footers, span/metrics LRU caches). The remaining wins come from:

1. **Reducing the number of files queries must touch** (write-side: optimizeWrite + intra-day compaction)
2. **Adding durable, multi-tier caching** so cold restarts and cross-pod queries don't repeatedly fetch the same immutable bytes from GCS
3. **Hiding necessary background work from the request path** (refresh, pre-warm)
4. **Making result caches version-aware** so they can hold longer without staleness risk

The plan is six stages. Stages can land independently; later stages build on earlier ones for full benefit.

**Expected outcome:** cold p99 trace lookup from ~300 ms → <50 ms, dashboard refresh load reduced via cache, no degradation on warm path or service-agnostic dashboard queries.

---

## 2. Diagnosis

### 2.1 What's already good

Existing setup found in `crates/scouter_dataframe/`:

- **SessionConfig tuned for GCS** (`src/storage.rs:162-231`): batch size 8192, 1MB Parquet footer hint, filter pushdown + reorder by selectivity, 64-way metadata concurrency, 4 parallel row-group writers.
- **Persistent SessionContext + catalog** (`src/parquet/tracing/engine.rs:241-242`, `catalog.rs:26-29`): single context with atomic table swaps — no "table not found" windows.
- **Hive partitioning by `partition_date` (YYYY-MM-DD)** (`engine.rs:101, 412`): aligns with the always-present time predicate in observability queries.
- **Bloom filters** on `trace_id`, `service_name`, `service_namespace`, `service_version`, `span_name` (`engine.rs:315-352`).
- **Z-ORDER** on `(start_time, service_name)` post-compaction (`engine.rs:437-440`) — clusters service data within files for clean stats-based pruning.
- **Data skipping stats columns** (`engine.rs:105-107`): `start_time`, `end_time`, `service_name`, `service_namespace`, `service_version`, `service_instance_id`, `duration_ms`, `status_code`, `partition_date` — Delta `_delta_log` carries min/max for these per file, enabling pruning *before* footer fetch.
- **Encoding** (`engine.rs:366-378`): ZSTD level 3, DELTA_BINARY_PACKED on `start_time`/`duration_ms`, dictionary on low-cardinality service columns.
- **CachingStore RAM cache** (`src/storage.rs`, `caching_store.rs`): 10K head entries (1h TTL), range cache capped at 2MB per entry.
- **Result caches** (`queries.rs:770-808`): span LRU 1K × 5min TTL, metrics LRU 500 × 60s TTL, both keyed with a `PLANNER_VERSION` for code-change invalidation.

### 2.2 Where time actually goes

A cold query (pod just started, no caches warm) goes through:

| Step | Mechanism | Typical cost |
|------|-----------|--------------|
| 1. TLS handshake / auth to GCS | Mostly amortised by `pool_max_idle_per_host=64` | <5 ms (warm pool) |
| 2. `_delta_log` read / `update_incremental` | List + JSON parse | 20–60 ms |
| 3. Partition + file-level pruning | In-memory from log stats — **no GCS fetch** | <1 ms |
| 4. Parquet footer fetch (per surviving file) | 1MB range request | 30–80 ms each |
| 5. Bloom filter / column index page fetch | Range request for filter pages | 30–80 ms each |
| 6. Row-group data pages | Range request for column chunks | 30–200 ms |
| 7. DataFusion plan + execute | In-memory | <5 ms |

Steps 4–6 are the bulk of cold latency, and they multiply with file count. This is exactly the regime where caching the small immutable bytes (footers, bloom pages) gives the highest ROI.

### 2.3 Why "DataFusion is fast" is misleading

DataFusion's query planner and executor are sub-ms on in-memory record batches. The 100s-of-ms tail is from object-store I/O. Comparable vendors handle the same physics differently:

| Vendor | Mechanism |
|--------|-----------|
| ClickHouse | Hot data on local NVMe (MergeTree); object store is a tier, not the primary. Sparse index always in RAM. |
| Datadog Husky | Iceberg on S3 + per-pod local SSD file cache (`foyer`/`cachelib` style). Footers/indexes cached aggressively. |
| Logfire | DataFusion + Parquet on object store (same as Scouter), with persistent disk cache + pre-warm + per-tenant worker pinning. |
| Tempo | Object store + memcached for bloom filters and TSDB-style index. |

The pattern is consistent: **persistent local cache for immutable bytes, plus eager pre-warm.** Pure RAM caches die with the pod and don't help cross-pod fan-out.

---

## 3. Implementation Stages

### Stage 1 — Reduce small-file count at the source

**Goal:** fewer files per partition_date = fewer footer fetches per query, even on the cold path.

**Background.** Compaction with Z-ORDER currently runs nightly. Throughout the day, writes accumulate as small files. Bloom filters and file-level stats still apply, but Z-ORDER clustering does not, and per-query footer-fetch count scales linearly with file count. This is the single biggest source of daytime tail latency.

**Changes:**

- Enable `delta.autoOptimize.optimizeWrite=true` on Delta table writers. Configure where Delta tables are created in `crates/scouter_dataframe/src/parquet/tracing/engine.rs:308-454` and the equivalent paths in `summary.rs`, `genai.rs`, `bifrost/`, `control/`. The writer combines small batches into properly-sized files at commit, reducing per-commit file count by 5–10×.
- Add an **hourly bin-packing compaction** job in `scouter-sql`, alongside the existing drift executor and agent poller. Use `optimize().with_type(OptimizeType::Compact)` (delta-rs API) — no Z-ORDER, just file consolidation. Cheap because no re-sorting.
- Keep nightly Z-ORDER on `(start_time, service_name)` unchanged.

**Verification:**
- File count per `partition_date` over 24h. Expect 5–10× reduction.
- p99 query latency on dashboard panels during business hours. Expect meaningful improvement.

**Risk:** low. Reversible by disabling the flag and stopping the hourly job. No schema changes. No client changes.

**Dependencies:** none.

---

### Stage 2 — Move `update_incremental` fully off the request path

**Goal:** no user query ever waits synchronously on a Delta log refresh.

**Background.** `SCOUTER_TRACE_REFRESH_INTERVAL_SECS` (default 10s) controls a background refresh that pulls new `_delta_log` entries to keep cross-pod views consistent. Some code paths may also trigger refreshes synchronously when stale, which would mean an unlucky user query stalls on a GCS LIST.

**Changes:**

- Audit every `update_incremental` call site:
  - `engine.rs:579`
  - `summary.rs:570`
  - `genai.rs:984`
  - `control/engine.rs:162, 295, 333`
  - `bifrost/engine.rs:357`
  - `bifrost/registry.rs:203`
- Confirm each is invoked from a background task (timer, write commit, compaction completion). Refactor any that can be reached from a request handler so the request returns from the snapshot in hand and the refresh is enqueued for the next background tick.
- Add a metric: `datafusion.refresh.on_request_path` — should be permanently zero.

**Verification:**
- Add a histogram around `update_incremental` duration and check it has no correlation with user request p99.
- Sustained ingestion test with a query workload — query p99 should not spike on refresh boundaries.

**Risk:** low. Pure plumbing. No semantic change to refresh cadence.

**Dependencies:** none.

---

### Stage 3 — Postgres-backed shared metadata cache

**Goal:** cross-pod, restart-survivable cache for footers, bloom filter pages, and column indexes. ~1–2 ms hits vs 50–200 ms GCS.

**Background.** Today's `CachingStore` is RAM-only, per-pod, and capped at 2MB per range. A new pod or a restarted pod re-fetches every footer from GCS even though sibling pods have already read them. Postgres is already in the architecture, has connection pools, handles BYTEA up to ~1GB with TOAST compression, and gives us a true shared cache without a new service to operate. Single-digit-ms primary-key lookup is 50–100× faster than GCS for the metadata-sized ranges (typically 10s of KB to a few MB).

**Schema:**

```sql
CREATE TABLE parquet_metadata_cache (
    object_path TEXT NOT NULL,
    range_start BIGINT NOT NULL,
    range_end   BIGINT NOT NULL,
    data        BYTEA NOT NULL,
    last_used   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (object_path, range_start, range_end)
);
CREATE INDEX parquet_metadata_cache_last_used_idx
    ON parquet_metadata_cache (last_used);
```

**Changes:**

- New ObjectStore wrapper in `crates/scouter_dataframe/src/parquet/cache/postgres_metadata_cache.rs`:
  - Implements `ObjectStore`. Delegates `get_range` to inner store on miss.
  - **Size threshold (4 MB):** only consults/populates Postgres for ranges below this. Data pages bypass Postgres entirely.
  - On miss: fetch from inner, write back to Postgres (best-effort, tolerate insert errors).
  - On hit: update `last_used` asynchronously (don't block read).
- Tier composition becomes: `RAM (CachingStore) → Postgres → GCS`. Stage 4 will insert a local-disk tier between RAM and Postgres.
- LRU eviction worker in `scouter-sql`, similar shape to the existing retention worker. Configurable max-rows or max-bytes; default to ~5 GB. Drop entries for paths absent from `_delta_log` (file was removed by VACUUM).
- New env vars:
  - `SCOUTER_METADATA_CACHE_ENABLED` (default `true`)
  - `SCOUTER_METADATA_CACHE_MAX_BYTES` (default `5_000_000_000`)
  - `SCOUTER_METADATA_CACHE_RANGE_MAX_BYTES` (default `4_194_304`)

**Verification:**
- Cold-pod p99 trace lookup against a pod whose siblings have populated Postgres. Expect 5–10× improvement vs today's cold path.
- Postgres load metrics (CPU, lock waits, WAL volume). Validate cap holds.
- Cache hit rate metric.

**Risk:** medium. Adds Postgres load. Mitigations:
- Hard size cap on entries (4 MB) means worst case ~1.25 M entries at 5 GB.
- Use a separate read-heavy connection pool if contention is observed.
- Feature-gate via env var — disabling restores prior behavior.

**Dependencies:** none in this codebase. Operationally requires the new table to be migrated.

---

### Stage 4 — Per-pod local disk cache (foyer)

**Goal:** pod-local cache for data pages (the bytes too large to put in Postgres). ~50–100 µs hits.

**Background.** Data pages (row group bytes) are too large for Postgres but benefit from cold-pod-local persistence. `foyer` is a Rust hybrid RAM+disk cache used in similar contexts (RisingWave). Files are immutable post-compaction, so cache key = object path + range; no consistency protocol needed.

**Changes:**

- Add `foyer` as a dependency (`Cargo.toml` workspace).
- New ObjectStore wrapper `DiskCacheStore` in `crates/scouter_dataframe/src/parquet/cache/disk_cache_store.rs`.
- Tier order: `RAM → DiskCache → PostgresMetadataCache → GCS`.
- Env-gated; default off so behavior is unchanged when unset:
  - `SCOUTER_DISK_CACHE_PATH` (e.g. `/var/cache/scouter`) — enables the tier.
  - `SCOUTER_DISK_CACHE_SIZE_GB` (default `5`).
- Deployment guidance in this document and in `docker-compose.yml` comments:
  - **Dev / single-pod:** `emptyDir` works. Cold-start cost on each pod restart, but Stages 3 and 5 mitigate.
  - **Production multi-pod:** `PersistentVolumeClaim` per pod (StatefulSet). Cache survives normal restarts. Reschedule to a new node still rebuilds — that's acceptable and bounded by Stage 5's pre-warm.

**Multi-pod semantics:**

- Each pod owns its own cache. Files are immutable, so duplicate copies across pods are safe and cheap.
- No coordination protocol. No invalidation. Just LRU eviction per pod.
- Cluster-wide hit rate emerges from the union of per-pod caches. Random/round-robin routing is fine; sticky routing could improve hit rate further but is unnecessary at current scale.

**Verification:**
- Warm-pod p99 on repeated dashboard queries — expect single-digit ms tail.
- Disk hit rate metric.
- Restart behavior: with PVC, hit rate immediately resumes; with `emptyDir`, observe rebuild curve.

**Risk:** low. Disabled by default. No schema changes. Files immutable, so no consistency concerns.

**Dependencies:** none. Stages 3 and 5 amplify the benefit but Stage 4 stands alone.

---

### Stage 5 — Pre-warm on startup + readiness gate

**Goal:** eliminate the cold-start cliff after pod restart or deploy.

**Background.** Even with Stages 3 + 4, a brand-new pod still pays the cold path on its first few queries. K8s readiness probes can hold traffic off the pod until its caches are warm enough.

**Changes:**

- New startup hook in the server binary: after `SessionContext` init, call `update_incremental` to materialize the active file list, then issue parallel `get_range` calls for footer + bloom filter regions of files in the last N partition_date directories. Uses the existing 1MB footer hint (`storage.rs:188`).
- Wire into the K8s readiness probe path: server reports "not ready" until pre-warm completes (or times out — with logged degraded status).
- Env vars:
  - `SCOUTER_PREWARM_DAYS` (default `2`)
  - `SCOUTER_PREWARM_TIMEOUT_SECS` (default `60`)
- Log structured metrics: files prewarmed, bytes prewarmed, duration.

**Verification:**
- Time-to-first-fast-query after pod restart: expect cold cliff to disappear once Stages 3 + 4 are in place.
- Readiness gate behavior under deliberate startup failure.

**Risk:** low. Falls back to "ready" on prewarm error so a misbehaving GCS doesn't block startup indefinitely.

**Dependencies:** materially benefits from Stages 3 and 4 being in place. Without them, the prewarmed bytes only live in RAM and die at the next pod restart.

---

### Stage 6 — Result cache improvements

**Goal:** longer cacheability for dashboard queries with safe, instant invalidation.

**Background.** The metrics result cache currently has a 60s TTL (`queries.rs:799-801`); span result cache has 5min TTL. Both already include `PLANNER_VERSION` in the key (`queries.rs:784`) for code-change invalidation. They do not include the Delta table version, so they cannot safely cache longer — any new commit could invalidate any result, but the cache doesn't know.

**Changes:**

- Include `delta_table.version()` in the cache key for both caches. New commits naturally produce new keys; old entries age out by LRU.
- Extend TTLs:
  - `metrics_cache`: 60s → 30 min
  - `span_cache`: 5 min → 1 h
- Keep "don't cache empty results" rule (`service.rs:562-615`) so newly-arriving data is reflected without delay.

**Verification:**
- Metrics cache hit rate during steady-state dashboard browsing — expect substantial increase.
- Validate invalidation: a fresh write should produce a new `delta_version`, so subsequent identical queries miss the old cache key.

**Risk:** low. Version-keyed invalidation is strictly safer than TTL-only.

**Dependencies:** none. Independent of Stages 1–5.

---

## 4. Out of Scope (with rationale)

- **`service_namespace` partitioning.** Partition pruning only applies when the query has a predicate on the partition column. For mixed observability workloads (drilldowns vs. service-agnostic dashboards), partitioning by `service_namespace` would help filtered queries but penalise unfiltered ones (more LIST calls, smaller files, worse compression). Z-ORDER on `(start_time, service_name)` plus `service_name` in `dataSkippingStatsColumns` already gives equivalent file-level pruning for filtered queries without imposing a directory cost on every other query. Revisit only if telemetry shows >70% of read traffic carries a `service_namespace =` predicate AND namespace cardinality stays under ~50.

- **Sticky / consistent-hash routing.** Would improve per-pod cache hit rate but adds routing-layer complexity. Overkill at current scale; revisit if pod count exceeds ~20.

- **Memcached / Redis shared cache.** Postgres metadata cache (Stage 3) covers the same use case using existing infrastructure. Adding another shared-cache service is unjustified.

- **Lifting the 2MB range cache cap globally.** Superseded by tiered design — Postgres for metadata, local disk (foyer) for data pages.

- **Custom DataFusion physical plan rewriting.** Would require deep DataFusion expertise for marginal gains. Existing pushdown, reorder, and partition pruning are already enabled.

---

## 5. Verification Approach

Each stage requires before/after benchmarks. Standard suite:

| Scenario | What it tests | Expected impact |
|----------|---------------|-----------------|
| Cold pod, single-trace lookup | Stages 3 + 4 + 5 | Largest improvement (300 ms → <50 ms p99) |
| Warm pod, dashboard refresh (repeated query) | Stage 6 | Sub-ms via result cache |
| Service-agnostic dashboard (no `service_name` filter) | Confirm Stage 1 helps; confirm we don't regress vs partitioning-by-service alternative | Should improve |
| Service-filtered drilldown | Confirms Z-ORDER + bloom + log-stats path | Should improve modestly |
| Multi-pod cold sibling | Stage 3 (cross-pod cache hit) | Pod B cold hit on Pod A's already-fetched footers |
| Pod restart with PVC | Stage 4 + 5 | Cache reattaches; pre-warm completes; no cliff |
| Pod restart with emptyDir | Stage 5 | Pre-warm rebuilds RAM + disk caches before readiness |

For each, capture: p50, p95, p99 wall time; GCS request count; cache hit rates per tier.

Per `AGENTS.md` verification rules, each PR runs:
- `make lints` at repo root (clippy `-D warnings`)
- `cargo test -p scouter-dataframe --all-features -- --nocapture --test-threads=1`
- `cargo test -p scouter-sql --all-features -- --nocapture --test-threads=1` (Stages 1, 3 touch SQL)
- `make test.dataframe`
- For Python-touching changes: `make setup.project` + `make lints` + relevant pytest

---

## 6. Suggested Rollout Order

| PR | Contents | Risk | Why this order |
|----|----------|------|----------------|
| 1 | Stage 1 + Stage 2 | Low | Write-side wins, no new infra, immediate file-count and refresh-stall reduction |
| 2 | Stage 3 (Postgres metadata cache) + schema migration | Medium | Cross-pod and restart-survivable benefits land together |
| 3 | Stage 4 (foyer disk cache) | Low | Env-gated default-off; safe to deploy ahead of operational adoption |
| 4 | Stage 5 (pre-warm + readiness) | Low | Builds on PR 2 + 3 to be fully effective |
| 5 | Stage 6 (result cache TTL + version key) | Low | Independent; can land any time |

PRs 2 + 3 are the highest-impact pair and should be prioritised after PR 1 lands and stabilises.

---

## 7. Open Questions

- **Postgres connection-pool sizing.** Stage 3 will add read traffic to Postgres. Validate `MAX_POOL_SIZE` is sufficient or carve out a separate pool for metadata cache reads.
- **PVC vs emptyDir default for production.** Recommendation here is PVC for production. Confirm with deployment owners.
- **Pre-warm fan-out cap.** A pod with 100K active files cannot prefetch all footers in <60s. Sample by recency / by service traffic. Define the policy in Stage 5 implementation.
- **delta-rs version / API surface.** Validate that `OptimizeType::Compact` (Stage 1) and `delta.autoOptimize.optimizeWrite` are supported in the pinned delta-rs version. Upgrade if needed.

---

## 8. References

- Existing code:
  - `crates/scouter_dataframe/src/storage.rs:162-231` (SessionConfig)
  - `crates/scouter_dataframe/src/parquet/tracing/engine.rs:241-454` (write path, compaction, refresh)
  - `crates/scouter_dataframe/src/parquet/tracing/queries.rs:770-808` (result caches)
  - `crates/scouter_dataframe/src/parquet/tracing/catalog.rs:26-29` (atomic table swaps)
- Environment variables: `AGENTS.md` § Server Environment Variables
- Verification commands: `AGENTS.md` § Verification After Any Code Change
