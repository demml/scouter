from scouter.tracing import TestSpanExporter, force_flush, get_tracer, init_tracer


def test_init_tracer():
    span_exporter = TestSpanExporter()
    init_tracer(name="test-service", exporter=span_exporter)
    tracer = get_tracer("test-tracer")

    with tracer.start_as_current_span(
        name="task_one",
        label="label_value",
        tags=[
            {"tag1": "foo"},
            {"tag2": "bar"},
        ],
    ) as span:
        span.set_attribute("task", "one")

    force_flush()

    assert len(span_exporter.spans) == 1
    assert len(span_exporter.baggage) == 2
    assert len(span_exporter.traces) == 1

    print(span_exporter.spans[0])
    a
