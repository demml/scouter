from scouter.tracing import init_tracer, get_tracer, TestSpanExporter, force_flush


def test_init_tracer():
    span_exporter = TestSpanExporter()
    init_tracer(name="test-service", exporter=span_exporter)
    tracer = get_tracer("test-tracer")

    with tracer.start_as_current_span(name="task_one", label="label_value") as span:
        span.set_attribute("task", "one")

    force_flush()

    assert len(span_exporter.spans) == 1
    assert len(span_exporter.baggage) == 0
    assert len(span_exporter.traces) == 1
