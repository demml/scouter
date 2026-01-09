import pytest
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig


@pytest.fixture()
def http_scouter_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture()
def kafka_scouter_server():
    with ScouterTestServer(kafka=True) as server:
        yield server


@pytest.fixture()
def kafka_scouter_openai_server():
    with ScouterTestServer(kafka=True, openai=True) as server:
        yield server


@pytest.fixture()
def rabbitmq_scouter_server():
    with ScouterTestServer(rabbit_mq=True) as server:
        yield server


@pytest.fixture()
def scouter_grpc_openai_server():
    with ScouterTestServer(openai=True) as server:
        init_tracer(
            service_name="tracing-grpc",
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("api-tracer"), server

        shutdown_tracer()
