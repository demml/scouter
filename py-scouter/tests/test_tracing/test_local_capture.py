from scouter.tracing import ScouterInstrumentor


def test_capture_round_trip(tracer):
    """enable → produce span → drain → non-empty list containing the expected span."""
    tracer.enable_local_capture()

    with tracer.start_as_current_span(name="capture_test"):
        pass

    spans = tracer.drain_local_spans()
    tracer.disable_local_capture()

    assert len(spans) >= 1, f"Expected at least 1 captured span, got {len(spans)}"
    assert any(s.span_name == "capture_test" for s in spans), (
        f"Expected a span named 'capture_test', got: {[s.span_name for s in spans]}"
    )


def test_capture_decorator_round_trip(tracer):
    """@tracer.span decorator also writes spans to the capture buffer."""
    tracer.enable_local_capture()

    @tracer.span("decorator_capture_test")
    def some_fn():
        return 42

    some_fn()

    spans = tracer.drain_local_spans()
    tracer.disable_local_capture()

    assert len(spans) >= 1, f"Expected at least 1 captured span, got {len(spans)}"
    assert any(s.span_name == "decorator_capture_test" for s in spans), (
        f"Expected a span named 'decorator_capture_test', got: {[s.span_name for s in spans]}"
    )


def test_drain_returns_empty_when_not_capturing(tracer):
    """drain returns empty list when capture is not enabled."""
    tracer.disable_local_capture()
    assert tracer.drain_local_spans() == []


def test_drain_clears_buffer(tracer):
    """Second drain after first returns empty."""
    tracer.enable_local_capture()
    tracer.drain_local_spans()  # first drain
    assert tracer.drain_local_spans() == []  # second drain empty
    tracer.disable_local_capture()


def test_disable_clears_buffer(tracer):
    """disable clears buffer — drain after disable returns empty."""
    tracer.enable_local_capture()
    tracer.disable_local_capture()
    assert tracer.drain_local_spans() == []


def test_enable_then_disable_cycle(tracer):
    """enable/disable cycle leaves capture off."""
    tracer.enable_local_capture()
    tracer.disable_local_capture()
    # After disable, drain should return empty regardless
    assert tracer.drain_local_spans() == []


def test_instrumentor_delegates():
    """ScouterInstrumentor methods delegate to BaseTracer."""
    inst = ScouterInstrumentor()
    inst.enable_local_capture()
    inst.disable_local_capture()
    assert inst.drain_local_spans() == []
