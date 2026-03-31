"""Tests for distributed tracing / W3C context propagation.

Covers:
- get_tracing_headers_from_current_span() emits W3C traceparent
- extract_span_context_from_headers() — W3C path and legacy fallback
- start_as_current_span(headers=...) links spans across service boundaries
- ScouterInstrumentor double-init warning
- ScouterTracingMiddleware server-span creation and header extraction
"""

import logging
import re

import pytest
from scouter.tracing import (
    ScouterInstrumentor,
    ScouterTracingMiddleware,
    extract_span_context_from_headers,
    get_tracing_headers_from_current_span,
)

_W3C_RE = re.compile(r"^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$")

# Fixed upstream identifiers used across multiple tests
_TRACE_ID = "4bf92f3577b34da6a3ce929d0e0e4736"
_SPAN_ID = "00f067aa0ba902b7"
_TRACEPARENT_SAMPLED = f"00-{_TRACE_ID}-{_SPAN_ID}-01"
_TRACEPARENT_UNSAMPLED = f"00-{_TRACE_ID}-{_SPAN_ID}-00"


# ─────────────────────────────────────────────────────────────────────────────
# get_tracing_headers_from_current_span
# ─────────────────────────────────────────────────────────────────────────────


def test_get_headers_includes_traceparent(tracer, span_exporter):
    span_exporter.clear()
    with tracer.start_as_current_span("header_test"):
        headers = get_tracing_headers_from_current_span()

    assert "traceparent" in headers, f"Expected 'traceparent' in {list(headers)}"
    assert _W3C_RE.match(headers["traceparent"]), f"traceparent does not match W3C format: {headers['traceparent']}"


def test_get_headers_retains_legacy_keys(tracer, span_exporter):
    span_exporter.clear()
    with tracer.start_as_current_span("legacy_keys"):
        headers = get_tracing_headers_from_current_span()

    assert "trace_id" in headers, "Legacy trace_id key must still be present"
    assert "span_id" in headers, "Legacy span_id key must still be present"


def test_traceparent_matches_active_span(tracer, span_exporter):
    """traceparent encodes the exact trace_id and span_id of the active span."""
    span_exporter.clear()
    with tracer.start_as_current_span("context_match") as span:
        headers = get_tracing_headers_from_current_span()
        sc = span.get_span_context()
        expected_trace = format(sc.trace_id, "032x")
        expected_span = format(sc.span_id, "016x")

    _, tp_trace, tp_span, _ = headers["traceparent"].split("-")
    assert tp_trace == expected_trace
    assert tp_span == expected_span


# ─────────────────────────────────────────────────────────────────────────────
# extract_span_context_from_headers
# ─────────────────────────────────────────────────────────────────────────────


def test_extract_w3c_sampled():
    result = extract_span_context_from_headers({"traceparent": _TRACEPARENT_SAMPLED})

    assert result is not None
    assert result["trace_id"] == _TRACE_ID
    assert result["span_id"] == _SPAN_ID
    assert result["is_sampled"] == "true"


def test_extract_w3c_not_sampled():
    result = extract_span_context_from_headers({"traceparent": _TRACEPARENT_UNSAMPLED})

    assert result is not None
    assert result["is_sampled"] == "false"


def test_extract_legacy_fallback():
    headers = {
        "trace_id": _TRACE_ID,
        "span_id": _SPAN_ID,
        "is_sampled": "true",
    }
    result = extract_span_context_from_headers(headers)

    assert result is not None
    assert result["trace_id"] == _TRACE_ID
    assert result["span_id"] == _SPAN_ID
    assert result["is_sampled"] == "true"


def test_extract_returns_none_for_empty_headers():
    assert extract_span_context_from_headers({}) is None


def test_extract_returns_none_for_invalid_traceparent():
    assert extract_span_context_from_headers({"traceparent": "not-a-valid-header"}) is None


def test_extract_w3c_preferred_over_legacy_keys():
    """When both traceparent and legacy keys are present, W3C wins."""
    other_trace = "aaaabbbbccccddddaaaabbbbccccdddd"
    result = extract_span_context_from_headers(
        {
            "traceparent": _TRACEPARENT_SAMPLED,
            "trace_id": other_trace,
            "span_id": "1111111111111111",
        }
    )
    assert result is not None
    assert result["trace_id"] == _TRACE_ID, "W3C traceparent must win over legacy keys"


# ─────────────────────────────────────────────────────────────────────────────
# start_as_current_span(headers=...) — cross-service span linking
# ─────────────────────────────────────────────────────────────────────────────


def test_w3c_headers_create_child_span(tracer, span_exporter):
    """A span started with headers={"traceparent": ...} is a child of the upstream span."""
    span_exporter.clear()
    with tracer.start_as_current_span("downstream", headers={"traceparent": _TRACEPARENT_SAMPLED}):
        pass

    assert len(span_exporter.spans) == 1
    span = span_exporter.spans[0]
    assert span.trace_id == _TRACE_ID, f"Expected trace_id={_TRACE_ID!r}, got {span.trace_id!r}"
    assert span.parent_span_id == _SPAN_ID, f"Expected parent_span_id={_SPAN_ID!r}, got {span.parent_span_id!r}"


def test_legacy_headers_create_child_span(tracer, span_exporter):
    """Legacy trace_id/span_id inside headers= dict also links the span."""
    span_exporter.clear()
    with tracer.start_as_current_span(
        "legacy_downstream",
        headers={"trace_id": _TRACE_ID, "span_id": _SPAN_ID},
    ):
        pass

    span = span_exporter.spans[0]
    assert span.trace_id == _TRACE_ID
    assert span.parent_span_id == _SPAN_ID


def test_w3c_headers_take_priority_over_explicit_params(tracer, span_exporter):
    """headers= traceparent wins over explicit trace_id/span_id positional params."""
    span_exporter.clear()
    other_trace = "bbbbccccddddeeeebbbbccccddddeeee"
    with tracer.start_as_current_span(
        "priority_check",
        headers={"traceparent": _TRACEPARENT_SAMPLED},
        trace_id=other_trace,
        span_id="abcdef1234567890",
    ):
        pass

    span = span_exporter.spans[0]
    assert span.trace_id == _TRACE_ID, f"W3C traceparent should win; got trace_id={span.trace_id!r}"


def test_child_span_gets_new_span_id(tracer, span_exporter):
    """The child span gets its own unique span_id, not the upstream's."""
    span_exporter.clear()
    with tracer.start_as_current_span("child", headers={"traceparent": _TRACEPARENT_SAMPLED}):
        pass

    span = span_exporter.spans[0]
    assert span.span_id != _SPAN_ID, "Child span must have its own span_id"


def test_round_trip_propagation(tracer, span_exporter):
    """Headers emitted from one span correctly link a downstream span."""
    span_exporter.clear()

    with tracer.start_as_current_span("upstream") as parent:
        outgoing = get_tracing_headers_from_current_span()
        sc = parent.get_span_context()
        parent_trace_id = format(sc.trace_id, "032x")
        parent_span_id = format(sc.span_id, "016x")

    span_exporter.clear()
    with tracer.start_as_current_span("downstream", headers=outgoing):
        pass

    child = span_exporter.spans[0]
    assert child.trace_id == parent_trace_id
    assert child.parent_span_id == parent_span_id


# ─────────────────────────────────────────────────────────────────────────────
# ScouterInstrumentor double-init warning
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.usefixtures("tracer")
def test_double_instrument_logs_warning(caplog):
    """_instrument() logs a warning when _provider is already set."""
    inst = ScouterInstrumentor()
    original = inst._provider  # noqa: SLF001
    try:
        inst._provider = object()  # noqa: SLF001  — simulate already-instrumented
        with caplog.at_level(logging.WARNING, logger="scouter.tracing"):
            inst._instrument()  # noqa: SLF001
        assert any(
            "already instrumented" in r.message.lower() for r in caplog.records
        ), f"Expected a double-init warning, got: {[r.message for r in caplog.records]}"
    finally:
        inst._provider = original  # noqa: SLF001


@pytest.mark.usefixtures("tracer")
def test_double_instrument_does_not_raise(caplog):
    """_instrument() when already instrumented returns cleanly without raising."""
    inst = ScouterInstrumentor()
    original = inst._provider  # noqa: SLF001
    try:
        inst._provider = object()  # noqa: SLF001
        with caplog.at_level(logging.WARNING, logger="scouter.tracing"):
            inst._instrument()  # must not raise
    finally:
        inst._provider = original  # noqa: SLF001


# ─────────────────────────────────────────────────────────────────────────────
# ScouterTracingMiddleware
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_middleware_creates_server_span(tracer, span_exporter):
    span_exporter.clear()

    async def app(scope, receive, send):
        pass

    scope = {"type": "http", "method": "GET", "path": "/health", "headers": []}
    await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1
    span = span_exporter.spans[0]
    assert span.span_name == "GET /health"
    assert span.span_kind == "SERVER"


@pytest.mark.asyncio
async def test_middleware_propagates_traceparent(tracer, span_exporter):
    """Middleware extracts traceparent from incoming ASGI headers and links the span."""
    span_exporter.clear()

    async def app(scope, receive, send):
        pass

    scope = {
        "type": "http",
        "method": "POST",
        "path": "/infer",
        "headers": [(b"traceparent", _TRACEPARENT_SAMPLED.encode())],
    }
    await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1
    span = span_exporter.spans[0]
    assert span.trace_id == _TRACE_ID
    assert span.parent_span_id == _SPAN_ID


@pytest.mark.asyncio
async def test_middleware_creates_root_span_without_traceparent(tracer, span_exporter):
    """Requests without traceparent get a fresh root span (no parent)."""
    span_exporter.clear()

    async def app(scope, receive, send):
        pass

    scope = {"type": "http", "method": "GET", "path": "/ping", "headers": []}
    await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    span = span_exporter.spans[0]
    assert span.parent_span_id is None


@pytest.mark.asyncio
async def test_middleware_skips_non_http_scope(tracer, span_exporter):
    """Non-HTTP/WebSocket scopes pass through without creating a span."""
    span_exporter.clear()
    reached = []

    async def app(scope, receive, send):
        reached.append(scope["type"])

    await ScouterTracingMiddleware(app, tracer=tracer)({"type": "lifespan"}, None, None)

    assert reached == ["lifespan"]
    assert len(span_exporter.spans) == 0


@pytest.mark.asyncio
async def test_middleware_websocket_creates_span(tracer, span_exporter):
    span_exporter.clear()

    async def app(scope, receive, send):
        pass

    scope = {"type": "websocket", "path": "/ws", "headers": []}
    await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1
    assert span_exporter.spans[0].span_kind == "SERVER"


# ─────────────────────────────────────────────────────────────────────────────
# extract_span_context_from_headers — additional edge cases
# ─────────────────────────────────────────────────────────────────────────────


def test_extract_legacy_fallback_defaults_is_sampled_to_false():
    """When is_sampled is absent from legacy headers, it defaults to 'false'."""
    result = extract_span_context_from_headers({"trace_id": _TRACE_ID, "span_id": _SPAN_ID})
    assert result is not None
    assert result["is_sampled"] == "false"


def test_extract_w3c_mixed_case_header_key():
    """HTTP/2 normalisation: 'Traceparent' (mixed-case) must be accepted."""
    result = extract_span_context_from_headers({"Traceparent": _TRACEPARENT_SAMPLED})
    assert result is not None
    assert result["trace_id"] == _TRACE_ID


def test_extract_legacy_invalid_hex_returns_none():
    """Legacy path must return None when trace_id is not valid hex."""
    result = extract_span_context_from_headers({"trace_id": "not-valid-hex!!!", "span_id": _SPAN_ID})
    assert result is None


# ─────────────────────────────────────────────────────────────────────────────
# ScouterTracingMiddleware — error propagation
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_middleware_span_closes_on_app_exception(tracer, span_exporter):
    """Span must be recorded even when the downstream app raises."""
    span_exporter.clear()

    async def app(scope, receive, send):
        raise RuntimeError("boom")

    scope = {"type": "http", "method": "POST", "path": "/fail", "headers": []}
    with pytest.raises(RuntimeError, match="boom"):
        await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1


# ─────────────────────────────────────────────────────────────────────────────
# ScouterInstrumentor lifecycle
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.usefixtures("tracer")
def test_instrumentor_uninstrument_resets_provider():
    """uninstrument() must clear _provider so is_instrumented returns False."""
    inst = ScouterInstrumentor()
    original = inst._provider  # noqa: SLF001
    try:
        # Simulate an instrumented state without touching real teardown
        sentinel = object()
        inst._provider = sentinel  # noqa: SLF001
        assert inst.is_instrumented
        inst._provider = None  # noqa: SLF001
        assert not inst.is_instrumented
    finally:
        inst._provider = original  # noqa: SLF001
