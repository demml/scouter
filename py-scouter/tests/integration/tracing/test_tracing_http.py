import time

from fastapi.testclient import TestClient
from scouter.client import ScouterClient
from scouter.tracing import get_tracing_headers_from_current_span

from .conftest import (  # type: ignore
    create_service_a_app,
    get_trace_by_id,
    get_traces_from_jaeger,
)


def test_tracer_http(setup_tracer_http):
    tracer, _server = setup_tracer_http

    @tracer.span("task_one")
    def task_one():
        time.sleep(0.02)

    @tracer.span("task_two_one")
    def task_two_one():
        time.sleep(0.05)

    @tracer.span("task_two")
    def task_two():
        task_two_one()
        time.sleep(0.05)

    for _ in range(10):
        with tracer.start_as_current_span("main_span"):
            task_one()
            task_two()

    traces = get_traces_from_jaeger("tracing-http")
    assert len(traces) > 0


def test_tracer_http_context(setup_tracer_http):
    tracer, _server = setup_tracer_http

    @tracer.span("task_one")
    def task_one():
        time.sleep(0.02)

    @tracer.span("task_two_one")
    def task_two_one():
        time.sleep(0.05)

    @tracer.span("task_two")
    def task_two():
        task_two_one()
        time.sleep(0.05)

    for _ in range(10):
        with tracer.start_as_current_span("main_span"):
            task_one()
            task_two()

    traces = get_traces_from_jaeger("tracing-http")
    assert len(traces) > 0


def test_distributed_trace_propagation(setup_tracer_http):
    """Test that traces propagate across services via HTTP headers"""
    tracer, _server = setup_tracer_http

    app = create_service_a_app()
    with TestClient(app) as remote_service_client:
        with tracer.start_as_current_span("client_span") as span:
            trace_id = span.trace_id
            span_id = span.span_id

            headers = get_tracing_headers_from_current_span()

            response = remote_service_client.post(
                "/calculate",
                json={"value": 5.0, "message": "test"},
                headers=headers,
            )

            assert response.status_code == 200
            result = response.json()
            assert result["result"] == 20.0
            assert "trace_id" in result
            assert result["trace_id"] == trace_id
            assert "span_id" in result
            assert result["span_id"] == span_id

    # Fetch the trace from Jaeger and verify spans
    time.sleep(2)  # Wait for trace to be exported
    trace_data = get_trace_by_id(trace_id)

    assert trace_data is not None

    # fetch trace spans from scouter
    scouter_client = ScouterClient()
    trace_span = scouter_client.get_trace_spans(trace_id)

    span = trace_span.get_span_by_name("service_a_inner_function")  # inner function called in service A
    assert span is not None
