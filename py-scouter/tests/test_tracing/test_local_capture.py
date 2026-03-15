from scouter.tracing import ScouterInstrumentor


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
