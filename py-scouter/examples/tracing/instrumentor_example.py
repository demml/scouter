"""
ScouterInstrumentor example.

ScouterInstrumentor registers Scouter as the global OpenTelemetry TracerProvider.
Any library that emits OTel spans (httpx, sqlalchemy, fastapi, etc.) will
automatically route those spans to Scouter — no per-library configuration needed.

Requirements:
    pip install scouter-ml opentelemetry-api

"""

from opentelemetry import trace
from scouter.tracing import BatchConfig, ScouterInstrumentor, get_tracer

# ---------------------------------------------------------------------------
# 1. Instrument once at application startup
# ---------------------------------------------------------------------------
#
# instrument() registers a Scouter-backed TracerProvider as the global OTel
# provider.  After this call, trace.get_tracer() anywhere in the process
# returns a tracer backed by Scouter.
#
# Key options:
#   transport_config  – where to send spans (GrpcConfig / HttpConfig).
#                       Defaults to the SCOUTER_SERVER_URI env var.
#   batch_config      – controls flush timing (scheduled_delay_ms) and
#                       queue sizes.  Tune for your latency / throughput.
#   sample_ratio      – fraction of traces to export (1.0 = 100%).
#   attributes        – key/value pairs stamped on every span produced by
#                       this provider (env, version, region, etc.).
#
# Call uninstrument() on shutdown to flush the export queue cleanly.

instrumentor = ScouterInstrumentor()
instrumentor.instrument(
    batch_config=BatchConfig(scheduled_delay_ms=500),
    sample_ratio=1.0,
    attributes={"env": "production", "service.version": "1.2.0"},
)

# ---------------------------------------------------------------------------
# 2. Use the standard OTel API — no Scouter import needed in business logic
# ---------------------------------------------------------------------------
#
# Because ScouterInstrumentor set the global provider, the standard OTel
# trace.get_tracer() call returns a tracer backed by Scouter.  Third-party
# libraries that are already OTel-instrumented will use this same provider
# transparently.

otel_tracer = trace.get_tracer("my-service")


def call_external_api(url: str) -> dict:
    """Simulate an outbound HTTP call with a child span."""
    with otel_tracer.start_as_current_span(
        "http.get", kind=trace.SpanKind.CLIENT
    ) as span:
        span.set_attribute("http.url", url)
        span.set_attribute("http.method", "GET")
        # ... real HTTP call here ...
        span.set_attribute("http.status_code", 200)
        return {"status": "ok"}


def query_database(query: str) -> list:
    """Simulate a DB query with a child span."""
    with otel_tracer.start_as_current_span(
        "db.query", kind=trace.SpanKind.CLIENT
    ) as span:
        span.set_attribute("db.system", "postgresql")
        span.set_attribute("db.statement", query)
        # ... real query here ...
        return []


# ---------------------------------------------------------------------------
# 3. Use the Scouter Tracer for decorator support
# ---------------------------------------------------------------------------
#
# get_tracer() returns a Scouter Tracer whose .span() decorator automatically
# creates a span around the decorated function.  Both the standard OTel API
# and the Scouter tracer export to the same provider set by instrument().

scouter_tracer = get_tracer("my-service")


@scouter_tracer.span("validate_payload")
def validate_payload(payload: dict) -> bool:
    """A simple decorated function — span is created and closed automatically."""
    return bool(payload)


def process_request(user_id: int, payload: dict) -> dict:
    """Context manager approach — access span attributes directly."""
    with scouter_tracer.start_as_current_span("process_request") as span:
        span.set_attribute("user.id", str(user_id))
        span.set_attribute("payload.size", len(str(payload)))

        validate_payload(payload)
        result = call_external_api("https://api.example.com/data")
        rows = query_database("SELECT * FROM events WHERE user_id = $1")

        span.add_event(
            "processing_complete",
            {"row_count": len(rows), "api_status": result["status"]},
        )
        return {"user_id": user_id, "rows": len(rows)}


# ---------------------------------------------------------------------------
# 4. Root span with baggage
# ---------------------------------------------------------------------------
#
# Baggage is a list of dicts propagated to downstream services via HTTP
# headers.  Each dict is a single key/value pair.


def handle_incoming_request(request_id: str, user_id: int) -> None:
    with scouter_tracer.start_as_current_span(
        "handle_request",
        baggage=[{"request_id": request_id}],
    ) as root:
        root.set_attribute("user.id", str(user_id))
        process_request(user_id, {"action": "purchase", "item_id": 42})

        print(f"trace_id: {root.trace_id}")
        print(f"span_id:  {root.span_id}")


# ---------------------------------------------------------------------------
# 5. Default attributes propagate to every span
# ---------------------------------------------------------------------------
#
# The attributes passed to instrument() ("env", "service.version") are
# automatically stamped on every span — including spans emitted by
# third-party libraries.  You do not need to set them manually.

# ---------------------------------------------------------------------------
# 6. Shutdown
# ---------------------------------------------------------------------------
#
# uninstrument() flushes the export queue and resets the global OTel provider.
# Call this during application shutdown (SIGTERM handler, lifespan event, etc.).


def shutdown() -> None:
    instrumentor.uninstrument()


# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    handle_incoming_request(request_id="req-abc-123", user_id=7)
    shutdown()
    print("Done. Spans exported to Scouter.")
