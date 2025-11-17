import pytest
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    HttpSpanExporter,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)


@pytest.fixture()
def http_scouter_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture(scope="session")
def http_span_exporter():
    """Create a fresh http span exporter for each test.
    This should write to the otel collector
    """
    return HttpSpanExporter()


@pytest.fixture(scope="session")
def setup_tracer_http(http_span_exporter):
    """Initialize tracer with http exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            name="test-service-http",
            exporter=http_span_exporter,
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("test-tracer"), server

        shutdown_tracer()


@pytest.fixture(scope="session")
def grpc_span_exporter():
    """Create a fresh grpc span exporter for each test.
    This should write to the otel collector
    """
    return GrpcSpanExporter()


@pytest.fixture(scope="session")
def setup_tracer_grpc(grpc_span_exporter):
    """Initialize tracer with grpc exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            name="test-service-grpc",
            exporter=grpc_span_exporter,
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("test-tracer"), server

        shutdown_tracer()
