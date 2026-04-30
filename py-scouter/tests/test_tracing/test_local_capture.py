import pytest
from scouter.mock import MockConfig
from scouter.tracing import ScouterInstrumentor, TestSpanExporter

RUN_ID = "local_capture_test"
RUN_BAGGAGE = [{"scouter.eval.run_id": RUN_ID}]


@pytest.fixture(autouse=True)
def capture_cleanup(tracer):
    yield
    tracer.disable_local_capture(RUN_ID)
    tracer.drain_local_spans(RUN_ID)


def test_capture_round_trip(tracer):
    """enable → produce span → drain → non-empty list containing the expected span."""
    tracer.enable_local_capture(RUN_ID)

    with tracer.start_as_current_span(name="capture_test", baggage=RUN_BAGGAGE):
        pass

    spans = tracer.drain_local_spans(RUN_ID)
    tracer.disable_local_capture(RUN_ID)

    assert len(spans) >= 1, f"Expected at least 1 captured span, got {len(spans)}"
    assert any(
        s.span_name == "capture_test" for s in spans
    ), f"Expected a span named 'capture_test', got: {[s.span_name for s in spans]}"


def test_capture_decorator_round_trip(tracer):
    """@tracer.span decorator also writes spans to the capture buffer."""
    tracer.enable_local_capture(RUN_ID)

    @tracer.span("decorator_capture_test", baggage=RUN_BAGGAGE)
    def some_fn():
        return 42

    some_fn()

    spans = tracer.drain_local_spans(RUN_ID)
    tracer.disable_local_capture(RUN_ID)

    assert len(spans) >= 1, f"Expected at least 1 captured span, got {len(spans)}"
    assert any(
        s.span_name == "decorator_capture_test" for s in spans
    ), f"Expected a span named 'decorator_capture_test', got: {[s.span_name for s in spans]}"


def test_drain_returns_empty_when_not_capturing(tracer):
    """drain returns empty list when capture is not enabled."""
    tracer.disable_local_capture(RUN_ID)
    assert tracer.drain_local_spans(RUN_ID) == []


def test_drain_clears_buffer(tracer):
    """Second drain after first returns empty."""
    tracer.enable_local_capture(RUN_ID)
    tracer.drain_local_spans(RUN_ID)  # first drain
    assert tracer.drain_local_spans(RUN_ID) == []  # second drain empty
    tracer.disable_local_capture(RUN_ID)


def test_disable_clears_buffer(tracer):
    """disable clears buffer — drain after disable returns empty."""
    tracer.enable_local_capture(RUN_ID)
    tracer.disable_local_capture(RUN_ID)
    assert tracer.drain_local_spans(RUN_ID) == []


def test_enable_then_disable_cycle(tracer):
    """enable/disable cycle leaves capture off."""
    tracer.enable_local_capture(RUN_ID)
    tracer.disable_local_capture(RUN_ID)
    # After disable, drain should return empty regardless
    assert tracer.drain_local_spans(RUN_ID) == []


def test_get_local_spans_by_trace_ids(tracer):
    """get_local_spans_by_trace_ids filters by trace ID without draining the buffer."""
    tracer.enable_local_capture(RUN_ID)

    with tracer.start_as_current_span("span_a", baggage=RUN_BAGGAGE) as span_a:
        trace_id = span_a.get_span_context().trace_id
    with tracer.start_as_current_span("span_b", baggage=RUN_BAGGAGE):
        pass

    trace_id_hex = format(trace_id, "032x")
    filtered = tracer.get_local_spans_by_trace_ids(RUN_ID, [trace_id_hex])
    assert any(
        s.span_name == "span_a" for s in filtered
    ), f"Expected span_a in filtered results, got: {[s.span_name for s in filtered]}"
    # buffer not drained — drain should return all captured spans
    all_spans = tracer.drain_local_spans(RUN_ID)
    assert len(all_spans) >= len(filtered)


@pytest.mark.usefixtures("tracer")
def test_instrumentor_delegates():
    """ScouterInstrumentor methods delegate to BaseTracer."""
    inst = ScouterInstrumentor()
    inst.enable_local_capture(RUN_ID)
    inst.disable_local_capture(RUN_ID)
    assert inst.drain_local_spans(RUN_ID) == []
    assert inst.get_local_spans_by_trace_ids(RUN_ID, ["invalid-id"]) == []


def test_child_span_inherits_run_id_from_parent_baggage(tracer):
    """Child span with no explicit baggage inherits scouter.eval.run_id from parent's baggage."""
    tracer.enable_local_capture(RUN_ID)

    with tracer.start_as_current_span("parent", baggage=RUN_BAGGAGE):
        with tracer.start_as_current_span("child"):  # no baggage kwarg
            pass

    spans = tracer.drain_local_spans(RUN_ID)
    tracer.disable_local_capture(RUN_ID)

    span_names = [s.span_name for s in spans]
    assert "parent" in span_names, f"Expected parent span, got: {span_names}"
    assert "child" in span_names, f"Expected child span, got: {span_names}"

    child_span = next(s for s in spans if s.span_name == "child")
    attr_keys = [a.key for a in child_span.attributes]
    assert (
        "scouter.eval.run_id" in attr_keys
    ), f"Expected scouter.eval.run_id on child span, got attributes: {attr_keys}"
    run_id_val = next(a.value for a in child_span.attributes if a.key == "scouter.eval.run_id")
    assert str(run_id_val) == RUN_ID


def test_third_party_style_span_inherits_run_id():
    """Span created via opentelemetry.trace global API inherits run_id from parent baggage.

    Requires ScouterInstrumentor (global provider) so that otel_trace.get_tracer() routes
    through Scouter's exporter and local capture buffer.
    """
    from opentelemetry import trace as otel_trace
    from scouter import trace as scouter_trace
    from scouter.tracing import ScouterInstrumentor

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    exporter = TestSpanExporter(batch_export=False)
    inst = ScouterInstrumentor()
    inst.instrument(transport_config=MockConfig(), exporter=exporter)
    tracer = scouter_trace.get_tracer("scouter-wrapper")

    try:
        tracer.enable_local_capture(RUN_ID)

        with tracer.start_as_current_span("wrapper", baggage=RUN_BAGGAGE):
            # Simulate a third-party library that calls global OTel API
            global_tracer = otel_trace.get_tracer("third-party-lib")
            with global_tracer.start_as_current_span("third-party-span"):
                pass

        spans = tracer.drain_local_spans(RUN_ID)
        tracer.disable_local_capture(RUN_ID)

        span_names = [s.span_name for s in spans]
        assert "third-party-span" in span_names, f"Expected third-party-span in captured spans, got: {span_names}"
        tp_span = next(s for s in spans if s.span_name == "third-party-span")
        attr_keys = [a.key for a in tp_span.attributes]
        assert "scouter.eval.run_id" in attr_keys, f"Expected scouter.eval.run_id on third-party span, got: {attr_keys}"
    finally:
        inst.uninstrument()
        ScouterInstrumentor._instance = None
        ScouterInstrumentor._provider = None
