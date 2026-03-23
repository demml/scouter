# Datasets

Reading and writing high volumes of data in an AI application should be easy, but it's not. The ecosystem is fragmented, and the engineering burden is high. You can use a managed data warehouse, but that adds latency and cost. You can write to object storage, but then you have to manage schemas, partitions, and query engines yourself. You can even write to logs, but that's a nightmare.

None of these give you what you actually want: define a schema,  push data, and query it later — without managing infrastructure.

## What Scouter Datasets Do

Scouter's dataset engine turns a Pydantic model into a Delta Lake table. You define the schema in Python, push records through a high-throughput queue, and the server handles storage, partitioning, and schema enforcement.

```
Pydantic model  →  Arrow schema  →  gRPC batches  →  Delta Lake
   (you define)     (automatic)      (background)     (server-side)
```

The write path is designed for production inference loops: sub-microsecond `insert()` calls, automatic batching, and backpressure handling. No `await`, no blocking, no GIL contention.

## Architecture

```mermaid
graph LR
    A[Your App] -->|insert record| B[DatasetProducer]
    B -->|channel send| C[Event Handler]
    C -->|batch full or timer| D[Arrow Batch Builder]
    D -->|IPC bytes| E[gRPC Client]
    E -->|insert_batch| F[Scouter Server]
    F -->|append| G[Delta Lake]
```

### How data flows

1. **You call `producer.insert(record)`** — serializes the Pydantic model to JSON via `model_dump_json()` and sends it through an unbounded channel. This returns immediately.

2. **Event handler receives the JSON** — pushes it onto an internal `ArrayQueue`. If the queue reaches `batch_size`, it triggers a flush.

3. **Background flush task wakes every 2 seconds** — if `scheduled_delay_secs` has elapsed since the last publish, it drains the queue regardless of size.

4. **Arrow batch is built** — `DynamicBatchBuilder` converts accumulated JSON rows into an Arrow `RecordBatch`. System columns are injected: `scouter_created_at`, `scouter_partition_date`, `scouter_batch_id`.

5. **IPC bytes sent via gRPC** — the batch is serialized to Arrow IPC format and sent to the server in a single gRPC call.

6. **Server appends to Delta Lake** — stored at the path derived from `catalog.schema_name.table`, partitioned by `scouter_partition_date`.

### Two background tasks

The `DatasetProducer` spawns two Tokio tasks on construction:

| Task | Trigger | Purpose |
|------|---------|---------|
| **Event handler** | Every `insert()` / `flush()` call | Processes channel events, triggers publish when batch is full |
| **Background flush** | Every 2s wake cycle | Publishes accumulated data if `scheduled_delay_secs` has elapsed |

Both tasks start before the constructor returns. If either fails to start within 10 seconds, the constructor raises an error.

## Schema-on-Write

Unlike append-only logging, Scouter enforces schema at write time. When you create a `TableConfig`, the Pydantic model's JSON Schema is converted to an Arrow schema and fingerprinted:

- **Any schema mismatch is caught before data lands** — the server verifies the fingerprint on every batch.
- **Schema changes produce a different fingerprint** — adding a field, removing a field, or changing a type all produce a new fingerprint. This prevents silent data corruption.
- **System columns are injected automatically** — you don't include them in your model.

### System columns

Every dataset table includes three columns managed by Scouter:

| Column | Type | Description |
|--------|------|-------------|
| `scouter_created_at` | `Timestamp(us, UTC)` | When the batch was built |
| `scouter_partition_date` | `Date32` | Today's date — used for Delta Lake partitioning |
| `scouter_batch_id` | `Utf8` (UUID v7) | Unique ID per batch — same for all rows in one flush |

### Supported types

| Python type | Arrow type | Notes |
|-------------|-----------|-------|
| `str` | `Utf8View` | |
| `int` | `Int64` | |
| `float` | `Float64` | |
| `bool` | `Boolean` | |
| `datetime` | `Timestamp(us, UTC)` | |
| `date` | `Date32` | |
| `Optional[T]` | nullable `T` | |
| `List[T]` | `List(T)` | Nested lists supported |
| `Enum` | `Dictionary(Int16, Utf8)` | String enum values |
| Nested `BaseModel` | `Struct(...)` | Recursive — up to 32 levels |

## Datasets and Monitoring

The dataset engine is a standalone system — it handles data storage and retrieval independent of Scouter's drift detection and alerting features.

That said, the two are designed to converge. In a future release, you will be able to write custom queries against data in your datasets and use the results to drive monitoring alerts. This means you can define domain-specific health checks (e.g., "alert if the 95th percentile prediction confidence drops below 0.7 over the last hour") directly on the data you're already storing — without maintaining a separate analytics pipeline.

## Next Steps

- [Quickstart](quickstart.md) — end-to-end example in 5 minutes
- [Writing Data](writing-data.md) — `DatasetProducer` configuration and patterns
- [Schema Reference](schema.md) — `TableConfig`, type mapping, fingerprinting
