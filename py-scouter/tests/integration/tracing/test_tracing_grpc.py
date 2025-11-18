import time

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
        with tracer.start_as_current_span("main_span"):
            task_one()
            task_two()

    traces = get_traces_from_jaeger("tracing-grpc")
    assert len(traces) > 0
