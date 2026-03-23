# Reading Data

## DatasetClient

`DatasetClient` is the read-side client for the dataset engine. It connects to a specific table, validates the schema fingerprint, and provides two ways to read data:

- **`read()`** -- returns validated Pydantic model instances. Best for application logic where you need typed objects.
- **`sql()`** -- executes arbitrary SQL and returns a `QueryResult` that converts to Arrow, Polars, or Pandas. Best for analytics and data exploration.

### Constructor

```python
from scouter.bifrost import DatasetClient, TableConfig
from scouter import GrpcConfig

client = DatasetClient(
    transport=GrpcConfig(server_uri="scouter.internal:50051"),
    table_config=TableConfig(
        model=PredictionRecord,
        catalog="production",
        schema_name="ml",
        table="predictions",
    ),
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `transport` | `GrpcConfig` | Yes | gRPC server connection settings |
| `table_config` | `TableConfig` | Yes | Schema, namespace, and fingerprint derived from your Pydantic model |

On construction, the client connects to the server and validates the schema fingerprint. If the table doesn't exist or the fingerprint doesn't match, the constructor raises an error.

!!! warning
    The `TableConfig` must use the same Pydantic model that was used to create the table. If your model has changed since the table was created, the fingerprint will not match.

## Strict Read

`read()` returns all rows from the bound table as validated Pydantic model instances.

```python
records = client.read()

for record in records:
    print(f"{record.user_id}: {record.prediction}")
```

With a row limit:

```python
recent = client.read(limit=100)
```

### How it works

1. Constructs `SELECT * FROM catalog.schema_name.table` (with optional `LIMIT`)
2. Executes via gRPC against DataFusion
3. Deserializes Arrow IPC bytes to a `pyarrow.Table`
4. Converts each row to a dict and calls `model.model_validate()` on it

This is the safest way to read data -- every row is validated against your Pydantic model, including type coercion and field validation. The trade-off is speed: Pydantic validation adds overhead compared to raw DataFrame conversion.

**When to use `read()`:**

- You need typed Python objects for application logic
- You want Pydantic validation guarantees
- Row counts are moderate (hundreds to low thousands)

**When to use `sql()` instead:**

- You need to filter, aggregate, or join before returning results
- You're doing analytics and want a DataFrame
- Result sets are large (thousands to millions of rows)

## SQL Queries

`sql()` executes a SQL query and returns a `QueryResult` -- a wrapper around Arrow IPC bytes.

```python
result = client.sql("SELECT * FROM production.ml.predictions WHERE confidence > 0.9")
```

The query runs on the server via Apache DataFusion. Tables are referenced by their three-part name: `catalog.schema_name.table`.

### Supported SQL

DataFusion supports standard SQL. Common patterns:

```python
# Filter
result = client.sql("""
    SELECT * FROM production.ml.predictions
    WHERE confidence > 0.9 AND model_name = 'credit_v2'
""")

# Aggregation
result = client.sql("""
    SELECT model_name, COUNT(*) as cnt, AVG(confidence) as avg_conf
    FROM production.ml.predictions
    GROUP BY model_name
""")

# CTE (Common Table Expression)
result = client.sql("""
    WITH recent AS (
        SELECT * FROM production.ml.predictions
        WHERE scouter_created_at > '2024-01-01T00:00:00Z'
    )
    SELECT model_name, COUNT(*) as cnt FROM recent
    GROUP BY model_name
""")

# Window function
result = client.sql("""
    SELECT user_id, prediction,
           ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY scouter_created_at DESC) as rn
    FROM production.ml.predictions
""")

# Cross-table JOIN
result = client.sql("""
    SELECT p.user_id, p.prediction, u.country
    FROM production.ml.predictions p
    JOIN production.ml.users u ON p.user_id = u.user_id
""")

# LIMIT and ORDER BY
result = client.sql("""
    SELECT * FROM production.ml.predictions
    ORDER BY confidence DESC
    LIMIT 100
""")
```

!!! note
    Only `SELECT` queries are allowed. `INSERT`, `UPDATE`, `DELETE`, `DROP`, and other DDL/DML statements are rejected by the server.

### System columns in queries

You can filter on system columns in your SQL:

```python
# Filter by date partition (efficient -- enables Delta Lake file pruning)
result = client.sql("""
    SELECT * FROM production.ml.predictions
    WHERE scouter_partition_date = '2024-03-15'
""")

# Filter by batch
result = client.sql("""
    SELECT * FROM production.ml.predictions
    WHERE scouter_batch_id = '0192f8a0-1234-7def-8abc-123456789abc'
""")

# Filter by creation time
result = client.sql("""
    SELECT * FROM production.ml.predictions
    WHERE scouter_created_at > '2024-03-15T10:00:00Z'
""")
```

Filtering on `scouter_partition_date` is the most efficient -- it enables Delta Lake partition pruning, which skips entire files that don't match the predicate.

## QueryResult

`sql()` returns a `QueryResult` object. This wraps the raw Arrow IPC bytes and provides zero-copy conversions to common DataFrame formats.

### to_arrow

```python
table = result.to_arrow()  # pyarrow.Table
```

Requires `pyarrow`. This is the most direct conversion -- Arrow IPC bytes are deserialized into a pyarrow Table with no intermediate copies.

### to_polars

```python
df = result.to_polars()  # polars.DataFrame
```

Requires `polars` and `pyarrow`. Internally calls `to_arrow()` then `polars.from_arrow()`, which uses the Arrow C Data Interface for zero-copy transfer.

### to_pandas

```python
df = result.to_pandas()  # pandas.DataFrame
```

Requires `pyarrow`. Internally calls `to_arrow()` then `table.to_pandas()`. This involves a copy (pandas uses its own memory layout), but the Arrow-to-pandas path is optimized by pyarrow.

### to_bytes

```python
raw = result.to_bytes()  # bytes
```

Returns the raw Arrow IPC stream bytes. Use this if you need to pass the data to another system, cache it, or use a library that reads Arrow IPC directly.

### Choosing a format

| Format | Dependencies | Copy | Best for |
|--------|-------------|------|----------|
| `to_arrow()` | `pyarrow` | Zero-copy | Interop with other Arrow-based tools |
| `to_polars()` | `polars`, `pyarrow` | Zero-copy | Fast analytics, lazy evaluation |
| `to_pandas()` | `pyarrow` | One copy | Compatibility with pandas ecosystem |
| `to_bytes()` | None | Zero-copy | Caching, forwarding, custom deserialization |

## Metadata

### list_datasets

List all registered dataset tables on the server:

```python
datasets = client.list_datasets()

for ds in datasets:
    print(f"{ds['catalog']}.{ds['schema_name']}.{ds['table']} ({ds['status']})")
```

Returns a list of dicts with keys: `catalog`, `schema_name`, `table`, `fingerprint`, `partition_columns`, `status`, `created_at`, `updated_at`.

### describe_dataset

Get detailed metadata for a specific table:

```python
info = client.describe_dataset("production", "ml", "predictions")

print(info["fingerprint"])
print(info["arrow_schema_json"])  # full Arrow schema as JSON
```

## Concurrent Access

`DatasetClient` is thread-safe. You can share a single client across multiple threads for concurrent queries:

```python
from concurrent.futures import ThreadPoolExecutor

client = DatasetClient(transport=GrpcConfig(), table_config=table_config)

def query_model(model_name: str):
    result = client.sql(f"""
        SELECT COUNT(*) as cnt FROM production.ml.predictions
        WHERE model_name = '{model_name}'
    """)
    return result.to_polars()

with ThreadPoolExecutor(max_workers=4) as pool:
    futures = [pool.submit(query_model, name) for name in ["v1", "v2", "v3", "v4"]]
    results = [f.result() for f in futures]
```

Writers (`DatasetProducer`) and readers (`DatasetClient`) can operate on the same table simultaneously. The server handles concurrency through Delta Lake's transaction log.

## Next Steps

- [Writing Data](writing-data.md) -- `DatasetProducer` configuration and patterns
- [Schema Reference](schema.md) -- type mapping, fingerprinting, `TableConfig` utilities
