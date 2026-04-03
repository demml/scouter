"""Hello service — port 8081.

Demonstrates that ScouterInstrumentor.instrument() overwrites the global
OpenTelemetry TracerProvider so that any OTel-aware framework calling
opentelemetry.trace.get_tracer_provider().get_tracer(name) automatically
routes spans to Scouter.

This is the pattern used by Google ADK (arize-ax example) and any other
framework that accepts a bring-your-own instrumentor:
    https://adk.dev/integrations/arize-ax/#configure-environment-variables

Flow
----
1. ScouterInstrumentor.instrument() sets Scouter as the global OTel provider.
2. The tracer is acquired via otel_trace.get_tracer_provider().get_tracer() —
   the same call path that Google ADK uses internally to create spans.
3. ScouterTracingMiddleware extracts the W3C traceparent from inbound requests
   and opens a Server span, linking this service's spans as children of the
   orchestrator span (same trace_id, correct parent_span_id).

Run:
    uvicorn hello_service:app --app-dir examples/tracing --port 8081
    # or: make example.tracing.hello

Requirements:
    pip install scouter-ml fastapi uvicorn httpx opentelemetry-api
"""

from contextlib import asynccontextmanager
from typing import Any, Optional

from fastapi import FastAPI
from opentelemetry import trace as otel_trace

from scouter.tracing import (
    BatchConfig,
    ScouterInstrumentor,
    ScouterTracingMiddleware,
    TracerProvider,
)
from scouter.transport import GrpcConfig


class _TracerProxy:
    """Deferred tracer — set in lifespan after instrument() establishes the
    global OTel provider.

    ScouterTracingMiddleware holds a reference to this proxy at app startup
    (before lifespan runs). The proxy transparently forwards all attribute
    access to the real tracer once lifespan has set _inner.
    """

    def __init__(self) -> None:
        self._inner: Optional[Any] = None

    def __getattr__(self, name: str) -> Any:
        if self._inner is None:
            raise RuntimeError("Tracer not initialized — instrument() must run first")
        return getattr(self._inner, name)


_instrumentor = ScouterInstrumentor()
_tracer = _TracerProxy()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Set Scouter as the global OTel provider, then acquire the tracer via
    the standard OTel API — the same call path Google ADK uses internally."""
    _instrumentor.instrument(
        transport_config=GrpcConfig(),
        batch_config=BatchConfig(scheduled_delay_ms=200),
        attributes={"service.name": "hello-service", "service.version": "1.0.0"},
    )

    # After instrument(), otel_trace.get_tracer_provider() returns the Scouter
    # TracerProvider. Google ADK (and any OTel-aware framework) calls this same
    # method internally — so ADK spans automatically flow to Scouter with zero
    # ADK-specific configuration.
    provider = otel_trace.get_tracer_provider()
    assert isinstance(provider, TracerProvider), (
        f"Expected Scouter TracerProvider, got {type(provider).__name__}."
    )
    _tracer._inner = provider.get_tracer("hello-service")

    yield
    _instrumentor.uninstrument()
    _tracer._inner = None


app = FastAPI(title="Hello Service", lifespan=lifespan)

# ScouterTracingMiddleware reads the W3C traceparent header on every inbound
# request and opens a Server span as the parent for all spans in this process.
# The orchestrator injects that header, so hello-service spans become children
# of the orchestrator.greet span (same trace_id, correct parent_span_id).
app.add_middleware(ScouterTracingMiddleware, tracer=_tracer)


@app.get("/hello")
async def hello(name: str = "World") -> dict:
    """Return a greeting. The active span here is already a child of the
    orchestrator's span (established by ScouterTracingMiddleware above)."""
    with _tracer.start_as_current_span("hello.build_greeting") as span:
        span.set_attribute("greeting.name", name)
        message = f"Hello, {name}!"
        span.set_attribute("greeting.message", message)

    return {"message": message}
