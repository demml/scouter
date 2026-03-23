# Quickstart

This guide takes you from zero to writing prediction data in under 5 minutes.

## Prerequisites

- `scouter-ml` installed (`pip install scouter-ml`)
- A running Scouter server with gRPC enabled (default port: 50051)

## 1. Define your schema

Your schema is a Pydantic `BaseModel`. Every field becomes an Arrow column in the underlying Delta Lake table.

```python
from pydantic import BaseModel
from typing import Optional
from datetime import datetime


class PredictionRecord(BaseModel):
    user_id: str
    model_name: str
    prediction: float
    confidence: float
    feature_1: float
    feature_2: float
    label: Optional[str] = None
    predicted_at: datetime
```

!!! note
    Do not include `scouter_created_at`, `scouter_partition_date`, or `scouter_batch_id` in your model. These system columns are injected automatically.

## 2. Create a TableConfig

`TableConfig` converts your Pydantic model into an Arrow schema and computes a fingerprint for schema enforcement.

```python
from scouter.dataset import TableConfig

table_config = TableConfig(
    model=PredictionRecord,  #(1)
    catalog="production",    #(2)
    schema_name="ml",
    table="credit_predictions",
    partition_columns=["model_name"],  #(3)
)

print(table_config.fqn)              # "production.ml.credit_predictions"
print(table_config.fingerprint_str)  # "a1b2c3d4..." (32-char hex)
```

1. Pass the class, not an instance.
2. `catalog.schema_name.table` forms the fully-qualified table name and determines the Delta Lake storage path.
3. Optional. These columns are used for server-side partitioning beyond the automatic `scouter_partition_date`.

## 3. Create a DatasetProducer

```python
from scouter.dataset import DatasetProducer, WriteConfig
from scouter import GrpcConfig

producer = DatasetProducer(
    table_config=table_config,
    transport=GrpcConfig(server_uri="localhost:50051"),
    write_config=WriteConfig(        #(1)
        batch_size=1000,
        scheduled_delay_secs=30,
    ),
)
```

1. `WriteConfig` is optional. Defaults: `batch_size=1000`, `scheduled_delay_secs=30`.

The constructor spawns two background tasks and verifies they start. If the gRPC server is unreachable, this will not fail — the producer lazily connects on first flush.

## 4. Insert records

```python
from datetime import datetime, timezone

for i in range(5000):
    record = PredictionRecord(
        user_id=f"user_{i}",
        model_name="credit_v2",
        prediction=0.85,
        confidence=0.92,
        feature_1=1.23,
        feature_2=4.56,
        predicted_at=datetime.now(timezone.utc),
    )
    producer.insert(record)  #(1)
```

1. `insert()` calls `record.model_dump_json()` and sends the JSON string through a channel. It does not block, does not acquire the GIL for I/O, and returns in under 1 microsecond.

Data is automatically batched and sent to the server when either condition is met:

- The internal queue reaches `batch_size` (1000 by default)
- `scheduled_delay_secs` (30s by default) have elapsed since the last publish

## 5. Shutdown

```python
producer.shutdown()  #(1)
```

1. Flushes any remaining data, cancels background tasks, and cleans up. Always call this on application exit.

## FastAPI Integration

The typical pattern for production use:

```python
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.dataset import DatasetProducer, TableConfig, WriteConfig
from scouter import GrpcConfig


class PredictionRecord(BaseModel):
    user_id: str
    prediction: float
    model_version: str


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.producer = DatasetProducer(
        table_config=TableConfig(
            model=PredictionRecord,
            catalog="prod",
            schema_name="ml",
            table="predictions",
        ),
        transport=GrpcConfig(server_uri="scouter.internal:50051"),
    )
    yield
    app.state.producer.shutdown()


app = FastAPI(lifespan=lifespan)


class PredictRequest(BaseModel):
    user_id: str
    features: dict


@app.post("/predict")
def predict(request: Request, payload: PredictRequest):
    prediction = model.predict(payload.features)  # your model

    # Non-blocking — returns immediately
    request.app.state.producer.insert(
        PredictionRecord(
            user_id=payload.user_id,
            prediction=prediction,
            model_version="v2.1",
        )
    )

    return {"prediction": prediction}
```

## Optional: Explicit Registration

The server auto-registers your dataset on first flush. If you want to register upfront (e.g., to verify connectivity during startup):

```python
status = producer.register()
print(status)                    # "registered" or "already_exists"
print(producer.is_registered)    # True
```

## Next Steps

- [Writing Data](writing-data.md) — batching behavior, backpressure, shutdown patterns
- [Schema Reference](schema.md) — type mapping, fingerprinting, `TableConfig` utilities
