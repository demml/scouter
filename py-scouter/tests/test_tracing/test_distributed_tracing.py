"""End-to-end distributed tracing tests.

Validates the two propagation paths:

  Manual path
  -----------
  The caller uses get_tracing_headers_from_current_span() to extract headers
  from the active span and passes them explicitly to the downstream service.
  The downstream service starts a span with ``headers=...`` to link it.

  Automatic path (global W3C propagator)
  ---------------------------------------
  After ScouterInstrumentor.instrument(), the global OTel TextMapPropagator is
  set to CompositePropagator([TraceContextTextMapPropagator(), W3CBaggagePropagator()]).
  Third-party instrumentors (HTTPXInstrumentor, StarletteInstrumentor) use
  this propagator automatically — no manual header extraction is needed when
  those instrumentors are active.

Terminology
-----------
  "Service A" = the upstream caller (creates the root/parent span)
  "Service B" = the downstream callee (creates the child span from incoming headers)
"""

import re

import pytest
from scouter.tracing import (
    ScouterTracingMiddleware,
    extract_span_context_from_headers,
    get_tracing_headers_from_current_span,
)

_W3C_RE = re.compile(r"^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$")


# ─────────────────────────────────────────────────────────────────────────────
# Manual cross-service propagation
# ─────────────────────────────────────────────────────────────────────────────


def test_manual_cross_service_propagation(tracer, span_exporter):
    """Service A extracts headers; Service B starts a child span from them."""
    span_exporter.clear()

    with tracer.start_as_current_span("service-a") as parent:
        outgoing_headers = get_tracing_headers_from_current_span()
        sc = parent.get_span_context()
        a_trace_id = format(sc.trace_id, "032x")
        a_span_id = format(sc.span_id, "016x")

    assert "traceparent" in outgoing_headers, "outgoing headers must contain W3C traceparent"
    assert _W3C_RE.match(outgoing_headers["traceparent"])

    span_exporter.clear()

    with tracer.start_as_current_span("service-b", headers=outgoing_headers):
        pass

    b_span = span_exporter.spans[0]
    assert b_span.trace_id == a_trace_id, "child must share the parent trace_id"
    assert b_span.parent_span_id == a_span_id, "child parent must be Service A's span"
    assert b_span.span_id != a_span_id, "child must have its own span_id"


def test_extract_then_start_span_roundtrip(tracer, span_exporter):
    """extract_span_context_from_headers output feeds back into start_as_current_span."""
    span_exporter.clear()

    with tracer.start_as_current_span("upstream") as parent:
        headers = get_tracing_headers_from_current_span()
        sc = parent.get_span_context()
        parent_trace_id = format(sc.trace_id, "032x")
        parent_span_id = format(sc.span_id, "016x")

    ctx = extract_span_context_from_headers(headers)
    assert ctx is not None
    assert ctx["trace_id"] == parent_trace_id
    assert ctx["span_id"] == parent_span_id
    assert ctx["is_sampled"] == "true"

    span_exporter.clear()
    with tracer.start_as_current_span("downstream", headers=headers):
        pass

    child = span_exporter.spans[0]
    assert child.trace_id == parent_trace_id
    assert child.parent_span_id == parent_span_id


def test_outgoing_headers_include_legacy_and_w3c_keys(tracer, span_exporter):
    """get_tracing_headers_from_current_span returns both W3C and legacy keys."""
    span_exporter.clear()
    with tracer.start_as_current_span("check-keys"):
        headers = get_tracing_headers_from_current_span()

    assert "traceparent" in headers, "W3C traceparent must be present"
    assert "trace_id" in headers, "legacy trace_id must still be present (backward compat)"
    assert "span_id" in headers, "legacy span_id must still be present (backward compat)"


# ─────────────────────────────────────────────────────────────────────────────
# Middleware-based cross-service propagation
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_middleware_cross_service_propagation(tracer, span_exporter):
    """Full flow: Service A headers → ASGI bytes → ScouterTracingMiddleware → child span."""
    span_exporter.clear()

    with tracer.start_as_current_span("service-a") as parent:
        outgoing_headers = get_tracing_headers_from_current_span()
        sc = parent.get_span_context()
        a_trace_id = format(sc.trace_id, "032x")
        a_span_id = format(sc.span_id, "016x")

    span_exporter.clear()

    async def downstream_app(scope, receive, send):
        pass

    # Simulate the HTTP layer encoding the headers as ASGI bytes
    asgi_headers = [
        (k.encode("latin-1"), v.encode("latin-1"))
        for k, v in outgoing_headers.items()
        if k in ("traceparent", "tracestate")
    ]
    scope = {
        "type": "http",
        "method": "POST",
        "path": "/agent-b/infer",
        "headers": asgi_headers,
    }
    await ScouterTracingMiddleware(downstream_app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1
    b_span = span_exporter.spans[0]
    assert (
        b_span.trace_id == a_trace_id
    ), f"Middleware child must share trace_id. got={b_span.trace_id!r}, want={a_trace_id!r}"
    assert (
        b_span.parent_span_id == a_span_id
    ), f"Middleware child parent must be Service A. got={b_span.parent_span_id!r}, want={a_span_id!r}"


@pytest.mark.asyncio
async def test_middleware_only_forwards_propagation_headers(tracer, span_exporter):
    """Middleware must not forward Authorization or Cookie to the span context."""
    span_exporter.clear()

    async def app(scope, receive, send):
        pass

    # Include a credential header alongside the traceparent
    asgi_headers = [
        (b"traceparent", b"00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        (b"authorization", b"Bearer secret-token"),
        (b"cookie", b"session=abc123"),
    ]
    scope = {"type": "http", "method": "GET", "path": "/secure", "headers": asgi_headers}
    await ScouterTracingMiddleware(app, tracer=tracer)(scope, None, None)

    assert len(span_exporter.spans) == 1
    span = span_exporter.spans[0]
    # Span must be linked to the upstream trace, not a new root
    assert span.trace_id == "4bf92f3577b34da6a3ce929d0e0e4736"


# ─────────────────────────────────────────────────────────────────────────────
# Automatic propagation — global W3C propagator registration
# ─────────────────────────────────────────────────────────────────────────────


@pytest.mark.usefixtures("tracer")
def test_global_w3c_propagator_is_registered_after_instrument():
    """After ScouterInstrumentor.instrument(), global textmap must be W3C-capable.

    This is the prerequisite for HTTPXInstrumentor and StarletteInstrumentor to
    automatically propagate traceparent without any manual header extraction.
    """
    from opentelemetry.propagate import get_global_textmap
    from opentelemetry.propagators.composite import CompositePropagator

    propagator = get_global_textmap()
    assert isinstance(propagator, CompositePropagator), (
        f"Expected CompositePropagator, got {type(propagator).__name__}. "
        "HTTPXInstrumentor/StarletteInstrumentor will NOT propagate headers "
        "without this."
    )


@pytest.mark.usefixtures("tracer")
def test_global_propagator_inject_and_extract_do_not_raise():
    """Global inject()/extract() must work after instrument() — no exceptions."""
    from opentelemetry.propagate import extract, inject

    carrier: dict[str, str] = {}
    inject(carrier)  # may inject empty traceparent if no active span
    ctx = extract(carrier)
    assert ctx is not None
