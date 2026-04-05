"""Orchestrator service — port 8000.

Demonstrates that ScouterInstrumentor.instrument() overwrites the global
OpenTelemetry TracerProvider so that any framework calling the OTel Python
API (e.g. Google ADK, StarletteInstrumentor, HTTPXInstrumentor) automatically
routes spans to Scouter — no Scouter-specific code required in the framework.

Flow
----
1. ScouterInstrumentor.instrument() calls opentelemetry.trace.set_tracer_provider()
   with a Scouter-backed TracerProvider.
2. opentelemetry.trace.get_tracer_provider().get_tracer(name) now returns a
   Scouter Tracer — the same path that Google ADK (and any OTel-aware framework)
   uses internally.
3. W3C traceparent headers are extracted from the active span and forwarded to
   hello-service and goodbye-service, which each run their own ScouterInstrumentor
   and continue the trace as children of the orchestrator span.

Note on automatic propagation
------------------------------
instrument() also registers a CompositePropagator (W3C TraceContext + Baggage)
as the global OTel TextMapPropagator.  Add opentelemetry-instrumentation-httpx
and call HTTPXInstrumentor().instrument() to skip the manual header extraction —
headers are injected automatically.  The manual approach is used here to make
the propagation path explicit.

Run:
    uvicorn examples.tracing.orchestrator_service:app --port 8000
    # or: make example.tracing.orchestrator

Then send a request:
    curl -X POST http://localhost:8000/greet \\
      -H 'Content-Type: application/json' \\
      -d '{"name": "Alice"}'

Requirements:
    pip install scouter-ml fastapi uvicorn httpx opentelemetry-api
"""

import asyncio
import os
from contextlib import asynccontextmanager
from typing import Any, Optional

import httpx
from fastapi import FastAPI
from opentelemetry import trace as otel_trace
from pydantic import BaseModel
from scouter.tracing import (
    BatchConfig,
    ScouterInstrumentor,
    TracerProvider,
    get_tracing_headers_from_current_span,
)
from scouter.transport import GrpcConfig

# ---------------------------------------------------------------------------
# Downstream service URLs — override via env vars for non-local deployments
# ---------------------------------------------------------------------------
HELLO_URL = os.getenv("HELLO_SERVICE_URL", "http://localhost:8001")
GOODBYE_URL = os.getenv("GOODBYE_SERVICE_URL", "http://localhost:8002")

_instrumentor = ScouterInstrumentor()

# Tracer is intentionally None until lifespan sets the global OTel provider.
# Frameworks like Google ADK do the same thing — they call
# otel_trace.get_tracer_provider().get_tracer(name) at request time, not at
# import time, which is why the provider must be set before any request arrives.
_tracer: Optional[Any] = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Set Scouter as the global OTel TracerProvider, then acquire a tracer via
    the standard OTel API — exactly the same path that Google ADK uses."""
    global _tracer

    _instrumentor.instrument(
        transport_config=GrpcConfig(),
        batch_config=BatchConfig(scheduled_delay_ms=200),
        attributes={"service.name": "orchestrator-service", "service.version": "1.0.0"},
    )

    # After instrument(), the global OTel provider IS the Scouter TracerProvider.
    # Any OTel-aware framework (Google ADK, StarletteInstrumentor, etc.) that calls
    # otel_trace.get_tracer_provider().get_tracer(...) will now get a Scouter Tracer.
    provider = otel_trace.get_tracer_provider()
    assert isinstance(provider, TracerProvider), (
        f"Expected Scouter TracerProvider, got {type(provider).__name__}. "
        "instrument() must be called before acquiring tracers."
    )
    _tracer = provider.get_tracer("orchestrator-service")

    yield
    _instrumentor.uninstrument()
    _tracer = None


app = FastAPI(title="Orchestrator Service", lifespan=lifespan)


# ---------------------------------------------------------------------------
# Request / response models
# ---------------------------------------------------------------------------


class GreetRequest(BaseModel):
    name: str


class GreetResponse(BaseModel):
    hello: str
    goodbye: str
    trace_id: str  # returned so you can look it up in Scouter immediately


# ---------------------------------------------------------------------------
# Endpoint
# ---------------------------------------------------------------------------


@app.post("/greet", response_model=GreetResponse)
async def greet(payload: GreetRequest) -> GreetResponse:
    """
    Fan out to hello-service and goodbye-service concurrently.

    Trace topology produced by this call:
        orchestrator.greet                  (this service, root span)
        ├── GET /hello   [hello-service]    (child — same trace_id)
        │   └── hello.build_greeting        (grandchild)
        └── GET /goodbye [goodbye-service]  (child — same trace_id)
            └── goodbye.build_farewell      (grandchild)
    """
    assert _tracer is not None, "Tracer not initialized — lifespan must run first"

    async with httpx.AsyncClient() as client:
        with _tracer.start_as_current_span("orchestrator.greet") as span:
            span.set_attribute("request.name", payload.name)

            # Extract the W3C traceparent for the active span.
            # Both downstream services run ScouterTracingMiddleware which reads
            # this header and continues the trace (child spans, same trace_id).
            propagation_headers = get_tracing_headers_from_current_span()

            hello_task = client.get(
                f"{HELLO_URL}/hello",
                params={"name": payload.name},
                headers=propagation_headers,
            )
            goodbye_task = client.get(
                f"{GOODBYE_URL}/goodbye",
                params={"name": payload.name},
                headers=propagation_headers,
            )
            hello_resp, goodbye_resp = await asyncio.gather(hello_task, goodbye_task)

            hello_resp.raise_for_status()
            goodbye_resp.raise_for_status()

            sc = span.get_span_context()
            trace_id = format(sc.trace_id, "032x")
            span.set_attribute("response.trace_id", trace_id)

            return GreetResponse(
                hello=hello_resp.json()["message"],
                goodbye=goodbye_resp.json()["message"],
                trace_id=trace_id,
            )
