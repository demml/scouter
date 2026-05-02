import asyncio
from typing import cast

import pytest
from opentelemetry import context as otel_context
from opentelemetry import trace
from opentelemetry.trace import (
    NonRecordingSpan,
    SpanContext,
    TraceFlags,
    TraceState,
    set_span_in_context,
)
from scouter.mock import MockConfig
from scouter.tracing import (
    ScouterInstrumentor,
    ScouterSpan,
    ScouterTracer,
    ScouterTracerProvider,
    TestSpanExporter,
    get_current_active_span,
    get_tracing_headers_from_current_span,
)


@pytest.fixture()
def instrumented_tracing():
    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    instrumentor = ScouterInstrumentor()
    exporter = TestSpanExporter(batch_export=False)
    instrumentor.instrument(
        transport_config=MockConfig(),
        exporter=exporter,
    )
    try:
        yield instrumentor, exporter
    finally:
        instrumentor.uninstrument()
        ScouterInstrumentor._instance = None
        ScouterInstrumentor._provider = None


def test_global_provider_is_scouter_provider(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    provider = trace.get_tracer_provider()
    assert isinstance(provider, ScouterTracerProvider)


def test_trace_get_tracer_returns_otel_tracer(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = trace.get_tracer("otel-compliance")
    assert isinstance(tracer, trace.Tracer)
    assert isinstance(tracer, ScouterTracer)


def test_span_is_otel_span_and_current_span_is_scouter_span(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = trace.get_tracer("otel-compliance")

    with tracer.start_as_current_span("outer") as span:
        assert isinstance(span, trace.Span)
        assert isinstance(span, ScouterSpan)
        assert span.is_recording()

        current = trace.get_current_span()
        assert isinstance(current, ScouterSpan)
        assert current.is_recording()


def test_parent_child_resolves_from_otel_context(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-compliance"))

    with tracer.start_as_current_span("parent") as parent:
        with tracer.start_as_current_span("child") as child:
            assert child.parent_context_id == parent.context_id


def test_span_end_marks_span_not_recording(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = trace.get_tracer("otel-compliance")
    span = tracer.start_span("manual-end")
    assert span.is_recording()
    span.end()
    assert not span.is_recording()


def test_span_end_clears_scouter_current_span(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-end-reset"))

    with tracer.start_as_current_span("root") as span:
        current = get_current_active_span()
        assert current.context_id == span.context_id
        span.end()
        assert not span.is_recording()

    with pytest.raises(Exception):
        get_current_active_span()


def test_explicit_empty_context_forces_root(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-empty-context"))

    with tracer.start_as_current_span("outer") as outer:
        with tracer.start_as_current_span("rooted", context=otel_context.Context()) as rooted:
            assert rooted.trace_id != outer.trace_id


def test_start_span_explicit_empty_context_forces_root(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-empty-context-manual"))

    with tracer.start_as_current_span("outer") as outer:
        rooted = tracer.start_span("manual-rooted", context=otel_context.Context())
        try:
            assert rooted.trace_id != outer.trace_id
        finally:
            rooted.end()


def test_start_span_end_does_not_clear_active_span(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-manual-end"))

    with tracer.start_as_current_span("outer") as outer:
        manual = tracer.start_span("manual-child")
        manual.end()

        current = get_current_active_span()
        assert current.context_id == outer.context_id


def test_asyncio_runner_cleanup_does_not_raise_valueerror(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing

    async def work() -> None:
        tracer = trace.get_tracer("otel-async")
        with tracer.start_as_current_span("async-span") as span:
            span.set_attribute("key", "value")

    runner = asyncio.Runner()
    try:
        runner.run(work())
        runner.run(work())
    finally:
        runner.close()


def test_span_decorator_sync_uses_scouter_current_span(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-decorator"))

    @tracer.span()
    def do_work(value: int) -> int:
        current = trace.get_current_span()
        assert isinstance(current, ScouterSpan)
        return value + 1

    assert do_work(1) == 2


def test_span_decorator_async_uses_scouter_current_span(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-decorator-async"))

    @tracer.span(record_args=True)
    async def do_work(value: int) -> int:
        current = trace.get_current_span()
        assert isinstance(current, ScouterSpan)
        return value + 1

    result = asyncio.run(do_work(1))
    assert result == 2


def test_start_as_current_span_decorates_sync_function(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-start-as-current"))

    @tracer.start_as_current_span("decorated-sync")
    def do_work() -> str:
        current = trace.get_current_span()
        assert isinstance(current, ScouterSpan)
        return "ok"

    assert do_work() == "ok"


def test_start_as_current_span_decorates_async_function(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-start-as-current-async"))

    @tracer.start_as_current_span("decorated-async")
    async def do_work() -> str:
        current = trace.get_current_span()
        assert isinstance(current, ScouterSpan)
        await asyncio.sleep(0)
        assert trace.get_current_span() is current
        return "ok"

    assert asyncio.run(do_work()) == "ok"


def test_explicit_otel_context_preserves_tracestate(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-tracestate"))

    parent_context = SpanContext(
        trace_id=int("1234567890abcdef1234567890abcdef", 16),
        span_id=int("1234567890abcdef", 16),
        is_remote=True,
        trace_flags=TraceFlags(0x01),
        trace_state=TraceState([("vendor", "value")]),
    )
    context = set_span_in_context(NonRecordingSpan(parent_context))

    with tracer.start_as_current_span("child", context=context) as child:
        child_context = child.get_span_context()
        assert child_context.trace_state["vendor"] == "value"
        headers = get_tracing_headers_from_current_span()
        assert headers["tracestate"] == "vendor=value"


def test_span_add_link_accepts_otel_span_context(instrumented_tracing):
    _instrumentor, _exporter = instrumented_tracing
    tracer = cast(ScouterTracer, trace.get_tracer("otel-link"))

    link_context = SpanContext(
        trace_id=int("fedcba0987654321fedcba0987654321", 16),
        span_id=int("fedcba0987654321", 16),
        is_remote=True,
        trace_flags=TraceFlags(0x01),
        trace_state=TraceState([("vendor", "value")]),
    )

    with tracer.start_as_current_span("child") as span:
        span.add_link(link_context, {"link.attr": "value"})


def test_child_span_inherits_baggage_from_parent_as_attribute(instrumented_tracing):
    """Child span started without explicit baggage gets parent's baggage as a span attribute."""
    from opentelemetry import baggage as otel_baggage
    from scouter import trace as scouter_trace

    _instrumentor, exporter = instrumented_tracing
    tracer = scouter_trace.get_tracer("otel-baggage-inherit")

    with tracer.start_as_current_span("parent", baggage=[{"test.run_id": "run-abc"}]):
        with tracer.start_as_current_span("child"):
            pass

    # Baggage should not leak outside the context manager
    assert otel_baggage.get_baggage("test.run_id") is None

    child_span = next(
        (s for s in exporter.spans if s.span_name == "child"),
        None,
    )
    assert child_span is not None, f"child span not found in: {[s.span_name for s in exporter.spans]}"
    attr_dict = {a.key: a.value for a in child_span.attributes}
    assert "test.run_id" in attr_dict, f"test.run_id not in child span attributes: {attr_dict}"
    assert attr_dict["test.run_id"] == "run-abc"


def test_empty_context_isolation_with_baggage(instrumented_tracing):
    """explicit context=Context() forces new root trace but still propagates baggage kwarg."""
    from scouter import trace as scouter_trace

    _instrumentor, _exporter = instrumented_tracing
    tracer = scouter_trace.get_tracer("otel-isolation-baggage")

    with tracer.start_as_current_span("outer") as outer:
        with tracer.start_as_current_span(
            "isolated",
            context=otel_context.Context(),
            baggage=[{"scouter.eval.run_id": "isolated-run"}],
        ) as isolated:
            assert isolated.trace_id != outer.trace_id, "isolated span must be in a new trace"
            from opentelemetry import baggage as otel_baggage

            val = otel_baggage.get_baggage("scouter.eval.run_id")
            assert val == "isolated-run", f"Expected baggage inside block, got: {val}"

    from opentelemetry import baggage as otel_baggage

    assert otel_baggage.get_baggage("scouter.eval.run_id") is None
