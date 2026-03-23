# Writing Data

## DatasetProducer

`DatasetProducer` is the write-side client for Bifrost. It maintains a persistent background queue that batches Pydantic model instances into Arrow RecordBatches and sends them to the server via gRPC.

### Constructor

```python
from scouter.bifrost import DatasetProducer, TableConfig, WriteConfig
from scouter import GrpcConfig

producer = DatasetProducer(
    table_config=TableConfig(
        model=MyModel,
        catalog="prod",
        schema_name="ml",
        table="predictions",
    ),
    transport=GrpcConfig(server_uri="scouter.internal:50051"),
    write_config=WriteConfig(batch_size=1000, scheduled_delay_secs=30),
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `table_config` | `TableConfig` | Yes | Schema, namespace, and fingerprint derived from your Pydantic model |
| `transport` | `GrpcConfig` | Yes | gRPC server connection settings |
| `write_config` | `WriteConfig` | No | Batching and flush timing. Defaults to `batch_size=1000`, `scheduled_delay_secs=30` |

### WriteConfig

Controls when data is flushed to the server.

```python
from scouter.dataset import WriteConfig

# Flush every 500 records or every 10 seconds, whichever comes first
config = WriteConfig(batch_size=500, scheduled_delay_secs=10)
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `batch_size` | 1000 | Number of records that triggers an immediate flush. Minimum: 1. |
| `scheduled_delay_secs` | 30 | Maximum seconds between flushes, regardless of queue size |

**Tuning guidance:**

- **High-throughput inference** (>1000 req/s): Use `batch_size=5000`, `scheduled_delay_secs=10`. Larger batches amortize gRPC overhead.
- **Low-throughput / latency-sensitive**: Use `batch_size=100`, `scheduled_delay_secs=5`. Data reaches the server faster.
- **Batch jobs**: Use `batch_size=10000`, `scheduled_delay_secs=60`. Maximize throughput.

## Inserting Records

```python
producer.insert(record)
```

`insert()` calls `record.model_dump_json()` to serialize the Pydantic model, then sends the JSON string through an unbounded Tokio channel. The call:

- Does not block
- Does not perform I/O
- Does not acquire the GIL for anything beyond the initial `model_dump_json()` call
- Returns in under 1 microsecond (after serialization)

### What happens after insert

The JSON string enters a `crossbeam::ArrayQueue` (capacity = `batch_size * 2`). When the queue reaches `batch_size`, the event handler triggers an immediate publish cycle:

1. Drain queue into a `Vec<String>`
2. Build an Arrow `RecordBatch` via `DynamicBatchBuilder`
3. Inject system columns (`scouter_created_at`, `scouter_partition_date`, `scouter_batch_id`)
4. Serialize to Arrow IPC bytes
5. Send via `insert_batch` gRPC call

### Backpressure

If the internal queue is full (producer is inserting faster than the gRPC client can flush), `insert()` retries with exponential backoff:

| Retry | Delay |
|-------|-------|
| 1 | 100ms |
| 2 | 200ms |
| 3 | 400ms |

After 3 retries, the insert raises an error. This is a signal that either:

- `batch_size` is too small for your throughput
- The server is unreachable or slow
- Network latency is high

## Flushing

```python
producer.flush()
```

Sends a flush signal through the event channel. The event handler will publish whatever is in the queue, even if `batch_size` hasn't been reached. This is non-blocking — it signals intent, it doesn't wait for completion.

Use `flush()` when you need data to reach the server before a specific point (e.g., end of a batch job, before a model swap).

## Shutdown

```python
producer.shutdown()
```

Graceful shutdown sequence:

1. Sends a `Flush` event through the channel
2. Waits 250ms for the event handler to process it
3. Cancels the event handler task
4. Waits 250ms for in-flight gRPC calls to complete
5. Aborts the event handler
6. Cancels and aborts the background flush task
7. Drops the channel sender

After `shutdown()`, calling `insert()` or `flush()` raises `AlreadyShutdown`.

!!! warning
    Always call `shutdown()` before your application exits. Without it, data in the queue is lost.

## Registration

```python
status = producer.register()
```

Explicitly registers the dataset table with the server. This is optional — the producer auto-registers on the first flush. Explicit registration is useful for:

- **Startup validation**: Verify the server is reachable and the schema is accepted
- **Schema conflict detection**: If a table with the same name but different schema exists, registration fails immediately rather than on first flush

The server verifies the schema fingerprint. If the table already exists with the same fingerprint, registration succeeds. If the fingerprint differs, it returns an error.

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `fingerprint` | `str` | 32-char hex schema fingerprint |
| `namespace` | `str` | Fully-qualified name (`catalog.schema.table`) |
| `is_registered` | `bool` | Whether the dataset has been registered with the server |
