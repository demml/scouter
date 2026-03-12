# Trace Storage Architecture

Scouter's server stores OTel spans in a **Delta Lake + Apache DataFusion** columnar engine, replacing Postgres-only trace storage. Spans are ingested at high throughput via a dual-actor write pipeline and queried via DataFusion SQL — giving you sub-second reads, efficient compaction, and cloud-native object store support.

---

## Write Path

```mermaid
graph TB
    subgraph "Client"
        A[Application Code] --> B[ScouterSpanExporter]
    end

    subgraph "Server Ingestion"
        B --> C[Transport Layer]
        C --> D[Consumer Worker]
        D --> E[MessageHandler]
    end

    subgraph "Storage Layer"
        E -->|"write_spans()"| F[Buffer Actor 10K cap · 5s flush]
        F -->|"TableCommand::Write"| G[Engine Actor Single writer]
        G -->|"build_batch() → append"| H[(Delta Lake trace_spans)]

        E --> I[TraceSummaryService write_summaries]
        I --> J[(Delta Lake trace_summaries)]
    end

    style H fill:#5c6bc0,color:#fff
    style J fill:#5c6bc0,color:#fff
```

Every span batch is processed by the `MessageHandler`, which fans out to two Delta Lake destinations:

1. **Delta Lake `trace_spans`** — full span data via the dual-actor buffer/engine pipeline
2. **Delta Lake `trace_summaries`** — one row per trace, updated as spans arrive via `TraceCache` (accumulates span updates and flushes per-trace summaries on flush)

---

## Component Reference

| Component | Crate | File | Purpose |
|-----------|-------|------|---------|
| `TraceSpanService` | `scouter_dataframe` | `parquet/tracing/service.rs` | Global singleton; owns both actors and the query service |
| `TraceSpanDBEngine` | `scouter_dataframe` | `parquet/tracing/engine.rs` | Delta Lake single-writer actor |
| `TraceSpanBatchBuilder` | `scouter_dataframe` | `parquet/tracing/engine.rs` | Zero-copy Arrow serialization of `TraceSpanRecord` |
| `TraceQueries` | `scouter_dataframe` | `parquet/tracing/queries.rs` | DataFusion query execution and DFS span tree assembly |
| `TraceSummaryService` | `scouter_dataframe` | `parquet/tracing/summary.rs` | Hour-bucketed summary table with cursor pagination |
| `CachingStore` | `scouter_dataframe` | `caching_store.rs` | `ObjectStore` wrapper; caches `head()` and small range reads (≤2 MB) for immutable Parquet files |
| `ObjectStore` | `scouter_dataframe` | `storage.rs` | Constructs the concrete cloud/local store wrapped in `CachingStore`; builds the tuned `SessionContext` |
| `Transport Layer (gRPC/HTTP/Kafka/RabbitMQ/Redis)` | `scouter_server` / `scouter_events` | `api/grpc/message.rs`, `api/routes/`, `events/` | Span ingestion handlers across all supported transports |
| `MessageHandler` | `scouter_sql` | `sql/postgres.rs` | Consumer worker; routes spans to `TraceSpanService` and `TraceSummaryService` |

---

## Write Path: Dual-Actor Design

`TraceSpanService` starts two long-lived Tokio tasks on initialization:

### Buffer Actor

Collects incoming `TraceSpanRecord` batches in memory and triggers a flush when either condition is met:

- **Capacity**: buffer reaches 10,000 spans
- **Time**: flush interval elapses (default: 5 seconds, configurable via `SCOUTER_TRACE_FLUSH_INTERVAL_SECS`)

On flush, the buffer actor drains itself and sends a `TableCommand::Write` message to the engine actor, then awaits acknowledgment via a oneshot channel.

### Engine Actor

The single writer for the Delta Lake table. It:

1. Receives `TableCommand::Write { spans, respond_to }`
2. Calls `build_batch()` on `TraceSpanBatchBuilder` to produce an Arrow `RecordBatch`
3. Calls `write_spans()` — acquires a write lock, refreshes the table state, appends the batch
4. Re-registers the updated table in the shared `SessionContext` so queries immediately see new data
5. Sends `Ok(())` back on the oneshot channel

**Why this pattern?** Delta Lake requires a single writer per table to avoid log conflicts. The two-actor design amortizes object-store I/O (many small span batches → fewer larger Parquet files per flush) while keeping the write lock duration minimal.

The engine actor also runs automatic compaction via an internal `tokio::time::interval` ticker:

```
loop {
    select! {
        cmd = rx.recv()          => handle Write / Optimize / Vacuum / Shutdown
        _ = compaction_ticker    => run Z-ORDER optimize automatically
    }
}
```

---

## Schema Design

### `trace_spans` (23 columns)

Stores one row per span. Hierarchy fields (`depth`, `span_order`, `path`, `root_span_id`) are **not stored** — they are computed at query time by `build_span_tree()` via Rust DFS traversal. This matches the Jaeger/Zipkin model and avoids ordering dependencies during ingest (spans may arrive out-of-order within a batch).

| Column | Arrow Type | Nullable | Notes |
|--------|-----------|----------|-------|
| `trace_id` | `FixedSizeBinary(16)` | No | W3C 128-bit trace ID |
| `span_id` | `FixedSizeBinary(8)` | No | W3C 64-bit span ID |
| `parent_span_id` | `FixedSizeBinary(8)` | Yes | Null on root spans |
| `flags` | `Int32` | No | W3C trace flags |
| `trace_state` | `Utf8` | No | W3C trace state header |
| `scope_name` | `Utf8` | No | Instrumentation scope |
| `scope_version` | `Utf8` | Yes | Instrumentation scope version |
| `service_name` | `Dictionary<Int32, Utf8>` | No | Dictionary-encoded; high repetition |
| `span_name` | `Utf8` | No | Operation name |
| `span_kind` | `Dictionary<Int8, Utf8>` | Yes | Dictionary-encoded; SERVER/CLIENT/etc. |
| `start_time` | `Timestamp(Microsecond, UTC)` | No | Sub-millisecond precision |
| `end_time` | `Timestamp(Microsecond, UTC)` | No | Sub-millisecond precision |
| `duration_ms` | `Int64` | No | Pre-computed for fast aggregation |
| `status_code` | `Int32` | No | OTel status code (0=Unset, 1=OK, 2=Error) |
| `status_message` | `Utf8` | Yes | |
| `label` | `Utf8` | Yes | Scouter-specific span label |
| `attributes` | `Map<Utf8, Utf8View>` | No | Key-value span attributes |
| `resource_attributes` | `Map<Utf8, Utf8View>` | Yes | Resource-level attributes |
| `events` | `List<Struct{name, timestamp, attributes, dropped_count}>` | No | Span events |
| `links` | `List<Struct{trace_id, span_id, trace_state, attributes, dropped_count}>` | No | Span links |
| `input` | `Utf8View` | Yes | Captured function input (JSON) |
| `output` | `Utf8View` | Yes | Captured function output (JSON) |
| `search_blob` | `Utf8View` | No | Pre-computed search string |
| `partition_date` | `Date32` | No | Hive partition column (days since Unix epoch); derived from `start_time` at ingest |

**Key design decisions:**

| Decision | Rationale |
|----------|-----------|
| Hierarchy not stored | Avoids ordering dependencies during ingest; DFS is computed in Rust at query time |
| `Dictionary` on `service_name`, `span_kind` | High repetition across spans → significant compression savings |
| `Utf8View` for `input`, `output`, `search_blob` | Large JSON payloads; Arrow `StringView` reduces heap copies for long strings |
| `search_blob` pre-computed at ingest | Concatenates service name, span name, scope name, attributes, and events into a single string — avoids JSON re-parsing on every attribute filter query |
| `Timestamp(Microsecond, UTC)` | Sub-millisecond precision with explicit timezone; matches OTel wire format |
| `FixedSizeBinary` for IDs | Compact binary representation; avoids hex string parsing in hot path |
| `partition_date` Hive partition | Enables partition pruning — queries filtered by time skip entire date directories without reading the Delta log |
| `DataSkippingStatsColumns` restricted | Delta Lake collects min/max stats only for `start_time`, `end_time`, `service_name`, `duration_ms`, `status_code`, `partition_date` — avoids inflating the Delta log with statistics on wide map/list columns |
| Bloom filters on `trace_id`, `service_name`, `span_name` | Written at Parquet row-group level; skip ~99% of row groups for equality lookups in the hot query path |

### `trace_summaries` (14 columns)

Stores one row per trace, hour-bucketed. Used exclusively for the paginated trace list — avoiding a full `trace_spans` scan for list views.

| Column | Arrow Type | Nullable |
|--------|-----------|----------|
| `trace_id` | `FixedSizeBinary(16)` | No |
| `service_name` | `Dictionary<Int32, Utf8>` | No |
| `scope_name` | `Utf8` | No |
| `scope_version` | `Utf8` | Yes |
| `root_operation` | `Utf8` | No |
| `start_time` | `Timestamp(Microsecond, UTC)` | No |
| `end_time` | `Timestamp(Microsecond, UTC)` | Yes |
| `duration_ms` | `Int64` | Yes |
| `status_code` | `Int32` | No |
| `status_message` | `Utf8` | Yes |
| `span_count` | `Int64` | No |
| `error_count` | `Int64` | No |
| `resource_attributes` | `Map<Utf8, Utf8View>` | Yes |

---

## Query Path Details

### `get_trace_spans`

Retrieves all spans for a single trace and assembles them into a hierarchy tree.

1. **Time filter applied first** — enables Delta Lake statistics-based file pruning (skips Parquet files whose `start_time` range doesn't overlap the query window)
2. **`trace_id` filter** — binary equality pushdown
3. **`RecordBatch` → `FlatSpan`** — zero-copy column extraction from Arrow arrays
4. **`build_span_tree()`** — Rust DFS traversal assigns `depth`, `span_order`, `path`, and `root_span_id` in-memory; orphan spans (parent not in batch) are appended at the end

### `get_trace_metrics`

Returns time-bucketed aggregates using a DataFusion CTE pipeline:

```sql
WITH
  -- Optional: pre-filter by attribute (LIKE on search_blob)
  matching_traces AS (
    SELECT DISTINCT trace_id FROM trace_spans
    WHERE start_time >= ? AND start_time < ?
    AND (search_blob LIKE '%key:value%' OR ...)
  ),
  -- Aggregate per-trace: duration = MAX(end_time) - MIN(start_time)
  trace_level AS (
    SELECT trace_id, MIN(start_time), MAX(end_time),
           MAX(CASE WHEN parent_span_id IS NULL THEN service_name END) AS root_service,
           MAX(status_code)
    FROM trace_spans WHERE ... GROUP BY trace_id
  ),
  service_filtered AS (...),
  bucketed AS (SELECT DATE_TRUNC('hour', trace_start), duration_ms, status_code ...)
SELECT
  bucket_start,
  COUNT(*) AS trace_count,
  AVG(duration_ms), approx_percentile_cont(duration_ms, 0.50),
  approx_percentile_cont(duration_ms, 0.95),
  approx_percentile_cont(duration_ms, 0.99),
  AVG(CASE WHEN status_code = 2 THEN 1.0 ELSE 0.0 END) AS error_rate
FROM bucketed GROUP BY bucket_start ORDER BY bucket_start
```

Attribute filters use `search_blob LIKE '%key:value%'` — no JSON parsing at query time.

### `get_paginated_traces`

Reads from `trace_summaries` using cursor-based pagination on `(start_time, trace_id)`. The cursor avoids full table scans on large datasets:

- **Forward**: `(start_time, trace_id) < (cursor_start, cursor_id) LIMIT n`
- **Backward**: `(start_time, trace_id) > (cursor_start, cursor_id) LIMIT n` (then reverse)

---

## Compaction and Maintenance

Trace data goes through a three-phase lifecycle:

### 1. Flush

The buffer actor writes small Parquet files on every flush (every 10K spans or 5 seconds). File sizes depend on span payload sizes but are typically small immediately after ingest.

### 2. Compaction (Z-ORDER)

The engine actor runs compaction automatically on a configurable interval (default: every 24 hours). Compaction uses Delta Lake Z-ORDER on `(start_time, service_name)`:

- **Target file size**: 128 MB
- **Z-ORDER columns**: `start_time` (time-range queries), `service_name` (service filter pushdown)
- **WriterProperties preserved**: bloom filters on `trace_id`, `service_name`, `span_name` are re-specified explicitly — without this, every compaction cycle would silently discard them from rewritten files

Z-ORDER co-locates spans with similar start times and service names within each Parquet file, maximizing the effectiveness of DataFusion's min/max statistics-based file pruning. After Z-ORDER, delta-encoded timestamps compress 4–8x within each row group.

Compaction is coordinated across pods via a **control table** — an optimistic-concurrency PostgreSQL table that ensures only one pod runs optimize or retention at a time, even in multi-replica deployments. The scheduler ticks every 5 minutes; the actual run schedule is persisted in `next_run_at` and survives pod restarts.

### 3. Retention

The engine actor can also run periodic data expiry. When `retention_days` is configured, it issues a logical delete against the `partition_date` partition column, then immediately vacuums the freed files. Because the predicate maps directly to a partition directory, Delta Lake skips all unaffected partitions.

Retention is also coordinated via the control table (task name: `trace_retention`), defaulting to a 24-hour schedule.

### 4. Vacuum

Removes old Parquet file versions that are no longer referenced by the Delta log, freeing object storage space. Vacuum runs automatically after both compaction and retention. It can also be triggered on-demand via `TraceSpanService::vacuum(retention_hours)`. Compaction is available on-demand via `TraceSpanService::optimize()`.

---

## Read/Write Performance Tuning

The optimization PR introduced several layers of read and write improvements that apply across all storage backends.

### CachingStore

Every storage backend (GCS, S3, Azure, local) is now wrapped in a `CachingStore`. After Z-ORDER compaction, Parquet files are immutable — the same path always returns the same bytes. `CachingStore` caches two call types:

- **`head()`** — up to 10,000 entries, 1-hour TTL. Eliminates repeated `HEAD` requests DataFusion issues to check file size before opening each file.
- **`get_range()`** — byte-addressed cache with a configurable max size (default 64 MB), 1-hour TTL. Only ranges ≤2 MB are cached; larger column-data reads (uncommon for footer-heavy queries) pass through uncached.

Cache size is configurable via `SCOUTER_OBJECT_CACHE_MB`. On GCS/S3 workloads where footer reads dominate query latency, this eliminates 1–2 round-trips (~30–60 ms each) per file per query.

### DataFusion Session Configuration

The `SessionContext` is now built with a tuned `SessionConfig` applied to all query and compaction operations:

| Setting | Value | Effect |
|---------|-------|--------|
| `target_partitions` | `available_parallelism` | Uses all CPU cores for parallel query execution |
| `batch_size` | 8192 | Controls Arrow RecordBatch size during query scans |
| `prefer_existing_sort` | true | Avoids re-sorting data that Z-ORDER already sorted |
| `parquet_pruning` | true | Enables min/max statistics-based file pruning |
| `collect_statistics` | true | Gathers column statistics to improve query planning |
| `pushdown_filters` | true | Pushes predicates into the Parquet reader — only matching rows are decoded |
| `reorder_filters` | true | Reorders predicates by selectivity — bloom filters (`trace_id`) evaluated before range checks (`start_time`) |
| `metadata_size_hint` | 1 MB | Fetches Parquet footer in one cloud round-trip; default 512KB is insufficient for files with bloom filters and page-level statistics |
| `bloom_filter_on_read` | true | Consults row-group bloom filters before decoding; explicit to guard against DataFusion version changes |
| `schema_force_view_types` | true | Reads `Utf8` columns as `Utf8View` — matches the schema's `StringView` type and avoids a downgrade on read |
| `meta_fetch_concurrency` | 64 | Parallel file stat operations when listing Delta table files; matches the HTTP connection pool size |
| `maximum_parallel_row_group_writers` | 4 | Encodes multiple row groups concurrently during compaction and flush |
| `maximum_buffered_record_batches_per_stream` | 8 | Smooths bursty GCS reads by buffering more decoded batches per stream |

### HTTP Connection Pooling

Cloud object stores (GCS, S3, Azure) are built with shared `ClientOptions`:

- Pool idle timeout: 120 seconds
- Max idle connections per host: 64 (matches `meta_fetch_concurrency`)
- Request timeout: 30 seconds
- Connect timeout: 5 seconds

### Parquet WriterProperties

Both flush writes and Z-ORDER compaction use the same `WriterProperties`:

| Setting | Value | Rationale |
|---------|-------|-----------|
| `max_row_group_size` | 32,768 rows | Creates ~4 row groups per 128 MB file; bloom filters and page statistics prune within files, not just across files |
| Bloom filter: `trace_id` | FPP 0.01, NDV 32,768 | Skips ~99% of row groups for `trace_id` equality lookups |
| Bloom filter: `service_name` | FPP 0.01, NDV 256 | Low cardinality but hot lookup path |
| Bloom filter: `span_name` | FPP 0.01, NDV 32,768 | High cardinality equality queries |
| Page stats: `start_time` | Page-level | Finest-grained time pruning within row groups |
| Page stats: `status_code` | Page-level | Prunes pages for error-only queries (no bloom filter — only 3 values) |
| Encoding: `start_time`, `duration_ms` | `DELTA_BINARY_PACKED` | 4–8x compression on near-sorted integers after Z-ORDER; 2–4x on durations within a service |
| Compression | ZSTD level 3 | ~40% better than SNAPPY on text columns; marginal decompression overhead is offset by reduced I/O |
| Dictionary: `span_name` | enabled | High repetition similar to `service_name` |

---

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `SCOUTER_STORAGE_URI` | `./scouter_storage` | Object store root. Supports `s3://`, `gs://`, `az://`, or local path |
| `SCOUTER_TRACE_COMPACTION_INTERVAL_HOURS` | `24` | How often automatic Z-ORDER compaction runs |
| `SCOUTER_TRACE_FLUSH_INTERVAL_SECS` | `5` | How often the span buffer flushes to Delta Lake |
| `SCOUTER_TRACE_BUFFER_SIZE` | `10000` | Span buffer capacity before a flush is triggered |
| `SCOUTER_OBJECT_CACHE_MB` | `64` | Maximum size of the in-process object store range cache (MB) |
| `AWS_REGION` | `us-east-1` | Required when using S3 storage |

---

## Storage Backends

The storage layer uses the `ObjectStore` abstraction, supporting all major cloud providers and local filesystems with no code changes. Every backend is wrapped in `CachingStore` — the caching layer is transparent to the rest of the system.

| Backend | URI Prefix | Notes |
|---------|-----------|-------|
| Local filesystem | `./path` or `/abs/path` | Default; good for development |
| Amazon S3 | `s3://bucket/prefix` | Requires `AWS_REGION` and standard AWS credentials |
| Google Cloud Storage | `gs://bucket/prefix` | Uses Application Default Credentials; also accepts `GOOGLE_ACCOUNT_JSON_BASE64` for service account key injection |
| Azure Blob Storage | `az://container/prefix` | Accepts both `AZURE_STORAGE_ACCOUNT_NAME`/`AZURE_STORAGE_ACCOUNT_KEY` (object_store convention) and `AZURE_STORAGE_ACCOUNT`/`AZURE_STORAGE_KEY` (az CLI/Terraform convention) |

The same Delta Lake protocol and DataFusion query engine run identically across all backends.
