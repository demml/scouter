import time

from opentelemetry import context as context_api
from scouter.client import ScouterClient, TraceFilters

from .conftest import _wait_for_export, get_traces_from_jaeger


def _wait_for_jaeger_traces(service_name: str, timeout_seconds: float = 10.0):
    deadline = time.time() + timeout_seconds
    traces = get_traces_from_jaeger(service_name)
    while len(traces) == 0 and time.time() < deadline:
        time.sleep(0.5)
        traces = get_traces_from_jaeger(service_name)
    return traces


def _wait_for_scouter_traces(service_name: str, timeout_seconds: float = 10.0):
    scouter_client = ScouterClient()
    deadline = time.time() + timeout_seconds
    response = scouter_client.get_paginated_traces(TraceFilters(service_name=service_name))
    while len(response.items) == 0 and time.time() < deadline:
        time.sleep(0.5)
        response = scouter_client.get_paginated_traces(TraceFilters(service_name=service_name))
    return response


def test_tracer_grpc(setup_tracer_grpc):
    tracer, _server = setup_tracer_grpc

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
        with tracer.start_as_current_span("main_span") as span:
            trace_id = span.trace_id
            task_one()
            task_two()

    traces = get_traces_from_jaeger("tracing-grpc")
    assert len(traces) > 0

    _wait_for_export()

    scouter_client = ScouterClient()
    trace_spans = scouter_client.get_trace_spans(trace_id)

    assert len(trace_spans.spans) > 0


def _test_tracer_grpc_sampling(setup_tracer_grpc_sample):
    tracer, _server = setup_tracer_grpc_sample
    service_name = "tracing-grpc-sample"
    total_traces = 60

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

    for _ in range(total_traces):
        token = context_api.attach(context_api.Context())
        try:
            with tracer.start_as_current_span("main_span"):
                task_one()
                task_two()
        finally:
            context_api.detach(token)

    _wait_for_export()

    traces = _wait_for_jaeger_traces(service_name)
    assert len(traces) > 0
    assert len(traces) < total_traces

    traces = _wait_for_scouter_traces(service_name)

    assert len(traces.items) > 0
    assert len(traces.items) < total_traces
