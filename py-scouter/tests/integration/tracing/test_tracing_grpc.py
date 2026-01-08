import time

from scouter.client import ScouterClient, TraceFilters

from .conftest import get_traces_from_jaeger  # type: ignore


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

    scouter_client = ScouterClient()
    trace_spans = scouter_client.get_trace_spans(trace_id)

    assert len(trace_spans.spans) > 0


def test_tracer_grpc_sampling(setup_tracer_grpc_sample):
    tracer, _server = setup_tracer_grpc_sample

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

    for _ in range(20):
        with tracer.start_as_current_span("main_span"):
            task_one()
            task_two()

    traces = get_traces_from_jaeger("tracing-grpc-sample")
    assert len(traces) > 0
    assert len(traces) < 20

    scouter_client = ScouterClient()
    traces = scouter_client.get_paginated_traces(
        TraceFilters(service_name="tracing-grpc-sample")
    )

    assert len(traces.items) > 0
    assert len(traces.items) < 20
