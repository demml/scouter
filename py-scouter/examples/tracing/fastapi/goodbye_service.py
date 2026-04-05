"""Goodbye service — port 8082.

Mirror of hello_service.py. Demonstrates that multiple independent services
can each call ScouterInstrumentor.instrument() (separate process, separate
singleton) and still participate in the same distributed trace via W3C
traceparent propagation.

Like hello_service, the tracer is acquired via
opentelemetry.trace.get_tracer_provider().get_tracer() after instrument() —
proving that the global OTel provider is now Scouter and any OTel-aware
framework (Google ADK, etc.) will route spans here automatically.

Run:
    uvicorn goodbye_service:app --app-dir examples/tracing --port 8082
    # or: make example.tracing.goodbye

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
    global OTel provider."""

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
    the standard OTel API."""
    _instrumentor.instrument(
        transport_config=GrpcConfig(),
        batch_config=BatchConfig(scheduled_delay_ms=200),
        attributes={"service.name": "goodbye-service", "service.version": "1.0.0"},
    )

    provider = otel_trace.get_tracer_provider()
    assert isinstance(provider, TracerProvider), f"Expected Scouter TracerProvider, got {type(provider).__name__}."
    _tracer._inner = provider.get_tracer("goodbye-service")

    yield
    _instrumentor.uninstrument()
    _tracer._inner = None


app = FastAPI(title="Goodbye Service", lifespan=lifespan)
app.add_middleware(ScouterTracingMiddleware, tracer=_tracer)


@app.get("/goodbye")
async def goodbye(name: str = "World") -> dict:
    """Return a farewell. Span is a child of the orchestrator's span."""
    with _tracer.start_as_current_span("goodbye.build_farewell") as span:
        span.set_attribute("farewell.name", name)
        message = f"Goodbye, {name}!"
        span.set_attribute("farewell.message", message)

    return {"message": message}
