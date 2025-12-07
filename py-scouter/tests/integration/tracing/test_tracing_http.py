import time

from .conftest import get_traces_from_jaeger  # type: ignore
from scouter.tracing import get_tracing_headers_from_current_span


def _test_tracer_http(setup_tracer_http):
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
        headers = get_tracing_headers_from_current_span()
        assert isinstance(headers, dict)

        print("Tracing Headers:", headers)
        time.sleep(0.05)

    @tracer.span("task_two")
    def task_two():
        task_two_one()
        time.sleep(0.05)

    for _ in range(10):
        with tracer.start_as_current_span("main_span"):
            task_one()
            task_two()
    a

    traces = get_traces_from_jaeger("tracing-http")
    assert len(traces) > 0
