"""End-to-end integration tests for the dataset engine.

Validates the full lifecycle: register → insert → flush → read / query.
Requires a running Scouter server (ScouterTestServer context manager).
"""

import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Optional

import pandas as pd
import polars as pl
import pyarrow as pa
import pytest
from pydantic import BaseModel
from scouter.bifrost import DatasetClient, DatasetProducer, TableConfig, WriteConfig
from scouter.mock import ScouterTestServer
from scouter.transport import GrpcConfig

# ---------------------------------------------------------------------------
# Test models
# ---------------------------------------------------------------------------


class PredictionRecord(BaseModel):
    model_name: str
    user_id: str
    score: float
    confidence: float
    label: Optional[str] = None


class UserFeatures(BaseModel):
    user_id: str
    age: int
    country: str
    score: float


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

FLUSH_WAIT_SECS = 8  # time for buffer flush + Delta write to complete


def _make_prediction(i: int, model_name: str = "model_a") -> PredictionRecord:
    return PredictionRecord(
        model_name=model_name,
        user_id=f"user_{i}",
        score=round(0.5 + (i % 50) / 100.0, 4),
        confidence=round(0.8 + (i % 20) / 100.0, 4),
        label="positive" if i % 2 == 0 else None,
    )


def _make_user_features(i: int) -> UserFeatures:
    return UserFeatures(
        user_id=f"user_{i}",
        age=20 + (i % 40),
        country="US" if i % 2 == 0 else "UK",
        score=round(0.1 + (i % 90) / 100.0, 4),
    )


def _setup_producer(
    model,
    catalog: str,
    schema_name: str,
    table: str,
    batch_size: int = 100,
) -> DatasetProducer:
    """Create and register a DatasetProducer."""
    config = TableConfig(
        model=model,
        catalog=catalog,
        schema_name=schema_name,
        table=table,
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
        write_config=WriteConfig(batch_size=batch_size, scheduled_delay_secs=1),
    )
    producer.register()
    return producer


def _setup_client(model, catalog: str, schema_name: str, table: str) -> DatasetClient:
    """Create a DatasetClient bound to a table."""
    config = TableConfig(
        model=model,
        catalog=catalog,
        schema_name=schema_name,
        table=table,
    )
    return DatasetClient(transport=GrpcConfig(), table_config=config)


def _insert_and_flush(producer: DatasetProducer, records: list) -> None:
    """Insert records and wait for them to land in Delta Lake."""
    for record in records:
        producer.insert(record)
    producer.flush()
    time.sleep(FLUSH_WAIT_SECS)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture()
def dataset_server():
    with ScouterTestServer() as server:
        yield server


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_dataset_full_lifecycle(dataset_server) -> None:
    """Register → insert → flush → read() → verify Pydantic models."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_lifecycle")
    records = [_make_prediction(i) for i in range(100)]
    _insert_and_flush(producer, records)

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_lifecycle")
    results = client.read()

    assert len(results) == 100
    for r in results:
        assert isinstance(r, PredictionRecord)

    producer.shutdown()


def test_dataset_to_arrow(dataset_server) -> None:
    """Query returns a pyarrow.Table with correct shape."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_arrow")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(50)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_arrow")
    result = client.sql("SELECT * FROM prod.ml.preds_arrow")
    table = result.to_arrow()

    assert isinstance(table, pa.Table)
    assert table.num_rows == 50
    assert "model_name" in table.column_names
    assert "score" in table.column_names

    producer.shutdown()


def test_dataset_to_polars(dataset_server) -> None:
    """Query returns a polars DataFrame with correct shape."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_polars")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(50)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_polars")
    result = client.sql("SELECT * FROM prod.ml.preds_polars")
    df = result.to_polars()

    assert isinstance(df, pl.DataFrame)
    assert df.height == 50
    assert "model_name" in df.columns

    producer.shutdown()


def test_dataset_to_pandas(dataset_server) -> None:
    """Query returns a pandas DataFrame with correct shape."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_pandas")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(50)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_pandas")
    result = client.sql("SELECT * FROM prod.ml.preds_pandas")
    df = result.to_pandas()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 50
    assert "model_name" in df.columns

    producer.shutdown()


def test_dataset_query_with_filter(dataset_server) -> None:
    """WHERE clause filters rows correctly."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_filter")

    records = [_make_prediction(i, "model_a") for i in range(50)]
    records += [_make_prediction(i, "model_b") for i in range(50, 100)]
    _insert_and_flush(producer, records)

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_filter")
    result = client.sql("SELECT * FROM prod.ml.preds_filter WHERE model_name = 'model_a'")
    table = result.to_arrow()

    assert table.num_rows == 50
    model_names = table.column("model_name").to_pylist()
    assert all(n == "model_a" for n in model_names)

    producer.shutdown()


def test_dataset_query_cte(dataset_server) -> None:
    """CTE (WITH clause) executes correctly."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_cte")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(100)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_cte")
    result = client.sql(
        """
        WITH high_scores AS (
            SELECT * FROM prod.ml.preds_cte WHERE score > 0.9
        )
        SELECT COUNT(*) as cnt FROM high_scores
    """
    )
    table = result.to_arrow()

    assert table.num_rows == 1
    count = table.column("cnt").to_pylist()[0]
    assert count > 0, f"CTE query returned {count} rows, expected > 0"

    producer.shutdown()


def test_dataset_query_aggregation(dataset_server) -> None:
    """GROUP BY aggregation works."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_agg")

    records = [_make_prediction(i, "model_a") for i in range(30)]
    records += [_make_prediction(i, "model_b") for i in range(30, 60)]
    _insert_and_flush(producer, records)

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_agg")
    result = client.sql(
        """
        SELECT model_name, COUNT(*) as cnt, AVG(score) as avg_score
        FROM prod.ml.preds_agg
        GROUP BY model_name
        ORDER BY model_name
    """
    )
    table = result.to_arrow()

    assert table.num_rows == 2
    names = table.column("model_name").to_pylist()
    assert "model_a" in names
    assert "model_b" in names

    producer.shutdown()


def test_dataset_cross_table_join(dataset_server) -> None:
    """JOIN across two dataset tables."""
    prod_producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_join")
    user_producer = _setup_producer(UserFeatures, "prod", "ml", "users_join")

    pred_records = [_make_prediction(i) for i in range(20)]
    user_records = [_make_user_features(i) for i in range(20)]

    _insert_and_flush(prod_producer, pred_records)
    _insert_and_flush(user_producer, user_records)

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_join")
    result = client.sql(
        """
        SELECT p.user_id, p.score AS pred_score, u.country
        FROM prod.ml.preds_join p
        JOIN prod.ml.users_join u ON p.user_id = u.user_id
    """
    )
    table = result.to_arrow()

    assert table.num_rows > 0
    assert "pred_score" in table.column_names
    assert "country" in table.column_names

    prod_producer.shutdown()
    user_producer.shutdown()


def test_dataset_list_and_describe(dataset_server) -> None:
    """list_datasets() and describe_dataset() return metadata."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_meta")
    _insert_and_flush(producer, [_make_prediction(0)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_meta")

    datasets = client.list_datasets()
    assert len(datasets) >= 1
    fqns = [f"{d['catalog']}.{d['schema_name']}.{d['table']}" for d in datasets]
    assert "prod.ml.preds_meta" in fqns

    info = client.describe_dataset("prod", "ml", "preds_meta")
    assert info["catalog"] == "prod"
    assert info["schema_name"] == "ml"
    assert info["table"] == "preds_meta"
    assert "arrow_schema_json" in info

    producer.shutdown()


def test_dataset_empty_result(dataset_server) -> None:
    """Query with impossible filter returns empty result, no error."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_empty")
    _insert_and_flush(producer, [_make_prediction(0)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_empty")
    result = client.sql("SELECT * FROM prod.ml.preds_empty WHERE score > 999.0")
    table = result.to_arrow()
    assert table.num_rows == 0

    producer.shutdown()


def test_dataset_sql_validation_error(dataset_server) -> None:
    """Non-SELECT SQL is rejected."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_sqlerr")
    producer.register()

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_sqlerr")

    with pytest.raises(RuntimeError, match="SELECT"):
        client.sql("DROP TABLE prod.ml.preds_sqlerr")

    producer.shutdown()


def test_dataset_idempotent_registration(dataset_server) -> None:
    """Registering the same schema twice returns already_exists."""
    config = TableConfig(
        model=PredictionRecord,
        catalog="prod",
        schema_name="ml",
        table="preds_idempotent",
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
    )

    status1 = producer.register()
    assert status1 == "created"

    status2 = producer.register()
    assert status2 == "already_exists"

    producer.shutdown()


def test_dataset_read_with_limit(dataset_server) -> None:
    """read(limit=N) returns at most N rows."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_limit")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(100)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_limit")
    results = client.read(limit=10)

    assert len(results) == 10
    for r in results:
        assert isinstance(r, PredictionRecord)

    producer.shutdown()


def test_dataset_concurrent_read_write(dataset_server) -> None:
    """Multiple writers and readers run concurrently without errors."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "preds_concurrent")
    # Seed initial data so readers have something to query
    _insert_and_flush(producer, [_make_prediction(i) for i in range(50)])

    client = _setup_client(PredictionRecord, "prod", "ml", "preds_concurrent")

    def writer(start_idx: int, count: int) -> None:
        for i in range(start_idx, start_idx + count):
            producer.insert(_make_prediction(i))
        producer.flush()

    def reader(n_queries: int) -> None:
        for _ in range(n_queries):
            result = client.sql("SELECT COUNT(*) as cnt FROM prod.ml.preds_concurrent")
            result.to_arrow()

    with ThreadPoolExecutor(max_workers=6) as pool:
        futures = []
        # 3 writers, each inserting 50 records
        for w in range(3):
            futures.append(pool.submit(writer, 100 + w * 50, 50))
        # 3 readers, each running 5 queries
        for _ in range(3):
            futures.append(pool.submit(reader, 5))

        # Collect results — raises if any thread hit an exception
        for f in as_completed(futures):
            f.result()

    # Final count should be at least initial 50
    time.sleep(FLUSH_WAIT_SECS)
    result = client.sql("SELECT COUNT(*) as cnt FROM prod.ml.preds_concurrent")
    table = result.to_arrow()
    final_count = table.column("cnt").to_pylist()[0]
    assert final_count >= 200  # seed(50) + 3 writers × 50 = 200 minimum

    producer.shutdown()


def test_dataset_client_unregistered_table(dataset_server):
    """DatasetClient constructor must raise when the table is not registered."""
    config = TableConfig(
        model=PredictionRecord,
        catalog="prod",
        schema_name="ml",
        table="does_not_exist",
    )
    with pytest.raises(RuntimeError):
        DatasetClient(transport=GrpcConfig(), table_config=config)


def test_dataset_client_fingerprint_mismatch(dataset_server):
    """DatasetClient constructor must raise on fingerprint mismatch."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "fp_mismatch_test")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(5)])

    # Build a TableConfig for a *different* model pointing at the same table
    wrong_config = TableConfig(
        model=UserFeatures,  # different schema → different fingerprint
        catalog="prod",
        schema_name="ml",
        table="fp_mismatch_test",
    )
    with pytest.raises(RuntimeError, match="[Ff]ingerprint"):
        DatasetClient(transport=GrpcConfig(), table_config=wrong_config)

    producer.shutdown()


def test_dataset_query_result_to_bytes(dataset_server):
    """QueryResult.to_bytes() must return bytes that round-trip through pyarrow."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "qr_bytes_test")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(10)])

    client = _setup_client(PredictionRecord, "prod", "ml", "qr_bytes_test")
    result = client.sql('SELECT * FROM "prod"."ml"."qr_bytes_test" LIMIT 10')

    raw = result.to_bytes()
    assert isinstance(raw, bytes)
    assert len(raw) > 0

    # Round-trip: bytes must form a valid Arrow IPC stream
    table = pa.ipc.open_stream(raw).read_all()
    assert table.num_rows == 10

    producer.shutdown()


def test_dataset_query_result_repr_len(dataset_server):
    """QueryResult __repr__ and __len__ must work as documented."""
    producer = _setup_producer(PredictionRecord, "prod", "ml", "qr_repr_test")
    _insert_and_flush(producer, [_make_prediction(i) for i in range(5)])

    client = _setup_client(PredictionRecord, "prod", "ml", "qr_repr_test")
    result = client.sql('SELECT * FROM "prod"."ml"."qr_repr_test"')

    assert len(result) > 0
    assert "QueryResult" in repr(result)

    producer.shutdown()
