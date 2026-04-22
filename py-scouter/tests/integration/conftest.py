import uuid
from contextlib import suppress
from pathlib import Path

import pytest
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    ScouterInstrumentor,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig


@pytest.fixture()
def isolated_server_config(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
):
    token = uuid.uuid4().hex[:12]
    monkeypatch.setenv("KAFKA_TOPIC", f"scouter_monitoring_{token}")
    monkeypatch.setenv("RABBITMQ_QUEUE", f"scouter_monitoring_{token}")
    monkeypatch.setenv("REDIS_CHANNEL", f"scouter_monitoring_{token}")
    return {"base_path": tmp_path / f"scouter_storage_{token}"}


def _cleanup_tracing_state() -> None:
    if ScouterInstrumentor._provider is not None or ScouterInstrumentor._instance is not None:
        with suppress(Exception):
            ScouterInstrumentor().uninstrument()

    with suppress(Exception):
        shutdown_tracer()

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None


@pytest.fixture(autouse=True)
def reset_scouter_tracing_state():
    _cleanup_tracing_state()
    yield
    _cleanup_tracing_state()


@pytest.fixture()
def http_scouter_server(isolated_server_config):
    with ScouterTestServer(**isolated_server_config) as server:
        yield server


@pytest.fixture()
def kafka_scouter_server(isolated_server_config):
    with ScouterTestServer(kafka=True, **isolated_server_config) as server:
        yield server


@pytest.fixture()
def kafka_scouter_openai_server(isolated_server_config):
    with ScouterTestServer(kafka=True, openai=True, **isolated_server_config) as server:
        yield server


@pytest.fixture()
def rabbitmq_scouter_server(isolated_server_config):
    with ScouterTestServer(rabbit_mq=True, **isolated_server_config) as server:
        yield server


@pytest.fixture()
def scouter_grpc_openai_server(isolated_server_config):
    with ScouterTestServer(openai=True, **isolated_server_config) as server:
        init_tracer(
            service_name="tracing-grpc",
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("api-tracer"), server

        shutdown_tracer()
