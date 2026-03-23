"""Performance benchmarks for the dataset engine.

Uses pytest-benchmark. Requires a running Scouter server (ScouterTestServer).
Run with: cd py-scouter && uv run pytest tests/integration/test_dataset_benchmarks.py -s -v
"""

import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Optional

import pytest
from pydantic import BaseModel
from scouter.bifrost import DatasetClient, DatasetProducer, TableConfig, WriteConfig
from scouter.mock import ScouterTestServer
from scouter.transport import GrpcConfig

FLUSH_WAIT_SECS = 8


class BenchRecord(BaseModel):
    model_name: str
    user_id: str
    score: float
    confidence: float
    label: Optional[str] = None


def _make_record(i: int) -> BenchRecord:
    return BenchRecord(
        model_name="model_a",
        user_id=f"user_{i}",
        score=round(0.5 + (i % 50) / 100.0, 4),
        confidence=round(0.8 + (i % 20) / 100.0, 4),
        label="positive" if i % 2 == 0 else None,
    )


@pytest.fixture(scope="module")
def bench_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture(scope="module")
def seeded_table(bench_server):
    """Register and seed a table with 1000 rows for read benchmarks."""
    config = TableConfig(
        model=BenchRecord,
        catalog="bench",
        schema_name="perf",
        table="records",
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
        write_config=WriteConfig(batch_size=500, scheduled_delay_secs=1),
    )
    producer.register()

    for i in range(1000):
        producer.insert(_make_record(i))
    producer.flush()
    time.sleep(FLUSH_WAIT_SECS)

    client = DatasetClient(transport=GrpcConfig(), table_config=config)

    yield producer, client

    producer.shutdown()


# ---------------------------------------------------------------------------
# Insert benchmarks
# ---------------------------------------------------------------------------


def test_benchmark_insert_100(bench_server, benchmark) -> None:
    """Insert 100 records + flush."""
    config = TableConfig(
        model=BenchRecord,
        catalog="bench",
        schema_name="perf",
        table="insert_100",
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
        write_config=WriteConfig(batch_size=100, scheduled_delay_secs=1),
    )
    producer.register()

    def insert_batch():
        for i in range(100):
            producer.insert(_make_record(i))
        producer.flush()

    benchmark(insert_batch)
    producer.shutdown()


def test_benchmark_insert_1k(bench_server, benchmark) -> None:
    """Insert 1000 records + flush."""
    config = TableConfig(
        model=BenchRecord,
        catalog="bench",
        schema_name="perf",
        table="insert_1k",
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
        write_config=WriteConfig(batch_size=1000, scheduled_delay_secs=1),
    )
    producer.register()

    def insert_batch():
        for i in range(1000):
            producer.insert(_make_record(i))
        producer.flush()

    benchmark(insert_batch)
    producer.shutdown()


# ---------------------------------------------------------------------------
# Query benchmarks (require seeded table)
# ---------------------------------------------------------------------------


def test_benchmark_query_to_arrow(seeded_table, benchmark) -> None:
    """sql().to_arrow() on 1K rows."""
    _, client = seeded_table

    def query():
        return client.sql("SELECT * FROM bench.perf.records").to_arrow()

    result = benchmark(query)
    assert result.num_rows == 1000


def test_benchmark_query_to_polars(seeded_table, benchmark) -> None:
    """sql().to_polars() on 1K rows."""
    _, client = seeded_table

    def query():
        return client.sql("SELECT * FROM bench.perf.records").to_polars()

    result = benchmark(query)
    assert result.height == 1000


def test_benchmark_query_to_pandas(seeded_table, benchmark) -> None:
    """sql().to_pandas() on 1K rows."""
    _, client = seeded_table

    def query():
        return client.sql("SELECT * FROM bench.perf.records").to_pandas()

    result = benchmark(query)
    assert len(result) == 1000


def test_benchmark_strict_read_100(bench_server, benchmark) -> None:
    """read() with Pydantic validation on 100 rows."""
    config = TableConfig(
        model=BenchRecord,
        catalog="bench",
        schema_name="perf",
        table="read_100",
    )
    producer = DatasetProducer(
        table_config=config,
        transport=GrpcConfig(),
        write_config=WriteConfig(batch_size=100, scheduled_delay_secs=1),
    )
    producer.register()

    for i in range(100):
        producer.insert(_make_record(i))
    producer.flush()
    time.sleep(FLUSH_WAIT_SECS)

    client = DatasetClient(transport=GrpcConfig(), table_config=config)

    def read_all():
        return client.read()

    results = benchmark(read_all)
    assert len(results) == 100
    producer.shutdown()


def test_benchmark_concurrent_4_readers(seeded_table, benchmark) -> None:
    """4 threads x 5 queries each."""
    _, client = seeded_table

    def concurrent_queries():
        def reader():
            for _ in range(5):
                client.sql("SELECT COUNT(*) as cnt FROM bench.perf.records").to_arrow()

        with ThreadPoolExecutor(max_workers=4) as pool:
            futures = [pool.submit(reader) for _ in range(4)]
            for f in as_completed(futures):
                f.result()

    benchmark(concurrent_queries)
