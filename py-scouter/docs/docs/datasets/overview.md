# Datasets

Reading and writing high volumes of data in an AI application should be easy, but it's not. The ecosystem is fragmented, and the engineering burden is high. You can use a managed data warehouse, but that adds latency and cost. You can write to object storage, but then you have to manage schemas, partitions, and query engines yourself.

None of these give you what you actually want: define a schema, push data, query it later -- without managing infrastructure.

## What Scouter Datasets Do

Scouter's dataset engine turns a Pydantic model into a Delta Lake table. You define the schema in Python, push records through a high-throughput queue, and query with SQL -- getting results back as Arrow tables, Polars DataFrames, Pandas DataFrames, or validated Pydantic model instances.

```
Write:  Pydantic model  →  Arrow schema  →  gRPC batches  →  Delta Lake
           (you define)      (automatic)       (background)      (server-side)

Read:   SQL query  →  DataFusion  →  Arrow IPC  →  Arrow / Polars / Pandas
           (you write)   (server)      (zero-copy)     (your choice)
```

The write path is designed for production inference loops: sub-microsecond `insert()` calls, automatic batching, and backpressure handling. No `await`, no blocking, no GIL contention.

The read path is designed for analytics: full SQL (joins, CTEs, window functions, aggregations) powered by Apache DataFusion, with zero-copy Arrow IPC delivery that converts directly to your preferred DataFrame library.

## Architecture

```mermaid
graph LR
    subgraph Write Path
        A[Your App] -->|insert record| B[DatasetProducer]
        B -->|channel send| C[Event Handler]
        C -->|batch full or timer| D[Arrow Batch Builder]
        D -->|IPC bytes| E[gRPC]
    end

    E -->|insert_batch| F[Scouter Server]
    F <-->|read/write| G[Delta Lake]

    subgraph Read Path
        H[DatasetClient] -->|sql / read| I[gRPC]
        I -->|query_dataset| F
        F -->|DataFusion| G
        I -->|QueryResult| H
        H -->|.to_arrow / .to_polars / .to_pandas| J[Your Analysis]
    end
```

### Write flow

1. **You call `producer.insert(record)`** -- serializes the Pydantic model to JSON and sends it through an unbounded channel. Returns immediately.

2. **Event handler receives the JSON** -- pushes it onto an internal `ArrayQueue`. If the queue reaches `batch_size`, it triggers a flush.

3. **Background flush task wakes periodically** -- if `scheduled_delay_secs` has elapsed since the last publish, it drains the queue regardless of size.

4. **Arrow batch is built** -- `DynamicBatchBuilder` converts accumulated JSON rows into an Arrow `RecordBatch`. System columns are injected automatically.

5. **IPC bytes sent via gRPC** -- the batch is serialized to Arrow IPC format and sent to the server.

6. **Server appends to Delta Lake** -- stored at the path derived from `catalog.schema_name.table`, partitioned by `scouter_partition_date`.

### Read flow

1. **You call `client.sql(query)`** -- sends the SQL string to the server via gRPC.

2. **Server executes via DataFusion** -- the query runs against the shared `SessionContext` where all registered tables are visible. Joins across tables, CTEs, window functions, and aggregations all work.

3. **Results returned as `QueryResult`** -- a wrapper around Arrow IPC bytes. Call `.to_arrow()`, `.to_polars()`, or `.to_pandas()` to convert.

For strict reads, `client.read()` constructs the SQL internally and returns validated Pydantic model instances.

### Two clients

| Client | Purpose | Lifecycle |
|--------|---------|-----------|
| `DatasetProducer` | Write records to a dataset table | Long-lived, background queue, call `shutdown()` on exit |
| `DatasetClient` | Read and query dataset tables | Stateless queries, bound to a table via `TableConfig` |

Both use gRPC transport and authenticate with the same `GrpcConfig`.

## Schema-on-Write

Unlike append-only logging, Scouter enforces schema at write time. When you create a `TableConfig`, the Pydantic model's JSON Schema is converted to an Arrow schema and fingerprinted:

- **Schema mismatch caught before data lands** -- the server verifies the fingerprint on every batch.
- **Schema changes produce a different fingerprint** -- adding, removing, or changing a field type all invalidate the fingerprint.
- **System columns injected automatically** -- you don't include them in your model.
- **Fingerprint validated on read** -- when you create a `DatasetClient` with a `TableConfig`, the client verifies the server's fingerprint matches your code.

### System columns

Every dataset table includes three columns managed by Scouter:

| Column | Type | Description |
|--------|------|-------------|
| `scouter_created_at` | `Timestamp(us, UTC)` | When the batch was built |
| `scouter_partition_date` | `Date32` | Today's date -- used for Delta Lake partitioning |
| `scouter_batch_id` | `Utf8` (UUID v7) | Unique ID per batch -- same for all rows in one flush |

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
| Nested `BaseModel` | `Struct(...)` | Recursive -- up to 32 levels |

## Datasets and Monitoring

The dataset engine is a standalone system -- it handles data storage and retrieval independent of Scouter's drift detection and alerting features.

That said, the two are designed to converge. In a future release, you will be able to write custom queries against data in your datasets and use the results to drive monitoring alerts. This means you can define domain-specific health checks (e.g., "alert if the 95th percentile prediction confidence drops below 0.7 over the last hour") directly on the data you're already storing -- without maintaining a separate analytics pipeline.

## Next Steps

- [Quickstart](quickstart.md) -- end-to-end write and read example
- [Writing Data](writing-data.md) -- `DatasetProducer` configuration and patterns
- [Reading Data](reading-data.md) -- `DatasetClient`, SQL queries, format conversions
- [Schema Reference](schema.md) -- `TableConfig`, type mapping, fingerprinting
