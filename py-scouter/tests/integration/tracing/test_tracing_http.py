import time


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
