# type: ignore

from datetime import datetime, timedelta

import pytest
import requests
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    HttpSpanExporter,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig


@pytest.fixture()
def http_span_exporter():
    """Create a fresh http span exporter for each test.
    This should write to the otel collector
    """
    return HttpSpanExporter()


@pytest.fixture()
def http_scouter_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture()
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


@pytest.fixture()
def grpc_span_exporter():
    """Create a fresh grpc span exporter for each test.
    This should write to the otel collector
    """
    return GrpcSpanExporter()


@pytest.fixture()
def setup_tracer_grpc(grpc_span_exporter):
    """Initialize tracer with grpc exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            service_name="tracing-grpc",
            transport_config=GrpcConfig(),
            exporter=grpc_span_exporter,
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        yield get_tracer("test-service-grpc"), server

        shutdown_tracer()


@pytest.fixture()
def setup_tracer_grpc_sample():
    """Initialize tracer with grpc exporter for each test."""
    with ScouterTestServer() as server:
        init_tracer(
            service_name="tracing-grpc-sample",
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            sample_ratio=0.2,
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


def get_trace_by_id(trace_id: str):
    """Helper function to get a specific trace from Jaeger by trace ID.

    Args:
        trace_id: The trace ID to query (hex string format)

    Returns:
        The trace data dictionary, or None if not found
    """
    JAEGER_TRACE_URL = f"http://localhost:16686/api/traces/{trace_id}"

    response = requests.get(JAEGER_TRACE_URL)
    response.raise_for_status()
    data = response.json()

    traces = data.get("data", [])

    # Return the first trace (should only be one for a specific trace ID)
    return traces[0] if traces else None


class ServiceARequest(BaseModel):
    value: float
    message: str


class ServiceAResponse(BaseModel):
    result: float
    trace_id: str
    span_id: str


def create_service_a_app() -> FastAPI:
    """
    Service A - Initiates the trace and calls Service B
    """
    app = FastAPI()
    tracer = get_tracer("service-a-tracer")

    def service_a_inner_function():
        with tracer.start_as_current_span("service_a_inner_function") as span:
            span.set_attribute("service_a.attribute", "value_a")

    @app.post("/calculate", response_model=ServiceAResponse)
    def calculate(request: Request, payload: ServiceARequest) -> ServiceAResponse:
        """Main endpoint for Service A"""
        incoming_headers = request.headers
        trace_id = incoming_headers.get("trace_id")
        span_id = incoming_headers.get("span_id")

        with tracer.start_as_current_span(
            "service_a_double", trace_id=trace_id, span_id=span_id
        ):
            doubled = payload.value * 2

            service_a_inner_function()

            return ServiceAResponse(
                result=doubled + 10, trace_id=trace_id, span_id=span_id
            )

    return app
