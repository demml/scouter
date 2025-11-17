# type: ignore

from datetime import datetime, timedelta

import pytest
import requests
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


@pytest.fixture(scope="module")
def http_span_exporter():
    """Create a fresh http span exporter for each test.
    This should write to the otel collector
    """
    return HttpSpanExporter()


@pytest.fixture(scope="module")
def setup_tracer_http(http_span_exporter):
    """Initialize tracer with http exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            service_name="tracing-http",
            exporter=http_span_exporter,
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("tracer-http"), server

        shutdown_tracer()


@pytest.fixture(scope="module")
def grpc_span_exporter():
    """Create a fresh grpc span exporter for each test.
    This should write to the otel collector
    """
    return GrpcSpanExporter()


@pytest.fixture(scope="module")
def setup_tracer_grpc(grpc_span_exporter):
    """Initialize tracer with grpc exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            service_name="tracing-grpc",
            exporter=grpc_span_exporter,
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("test-service-grpc"), server

        shutdown_tracer()


def get_traces_from_jaeger(service_name: str):
    """Helper function to get traces from jaeger."""
    JAEGER_QUERY_URL = "http://localhost:16686/api/traces"

    now = datetime.now()
    five_minutes_ago = now - timedelta(minutes=5)

    end_time_micros = int(now.timestamp() * 1_000_000)
    start_time_micros = int(five_minutes_ago.timestamp() * 1_000_000)

    params = {
        "service": service_name,
        "start": start_time_micros,
        "end": end_time_micros,
        "limit": 100,  # Max number of traces to fetch
        "lookback": "5m",
    }
    response = requests.get(JAEGER_QUERY_URL, params=params)
    response.raise_for_status()
    data = response.json()

    traces = data.get("data", [])

    return traces
