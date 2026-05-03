import pytest
from pydantic import BaseModel
from scouter.mock import MockConfig
from scouter.tracing import (
    ScouterInstrumentor,
    TestSpanExporter,
    get_tracer,
    shutdown_tracer,
)


class ChatInput(BaseModel):
    message: str
    user_id: int


class EventData(BaseModel):
    foo: str
    bar: int
    baz: float


def _reset_tracing_state() -> None:
    """Reset both the Rust provider store and Python OTel globals."""
    try:
        instrumentor = ScouterInstrumentor()
        if instrumentor.is_instrumented:
            instrumentor.uninstrument()
    except Exception:  # noqa: BLE001
        pass
    try:
        shutdown_tracer()
    except Exception:  # noqa: BLE001
        pass


@pytest.fixture()
def span_exporter():
    """Create a fresh test span exporter for each test."""
    return TestSpanExporter(batch_export=False)


@pytest.fixture()
def tracer(span_exporter):
    """Initialize an isolated low-level tracer for each test."""
    _reset_tracing_state()
    ScouterInstrumentor().instrument(
        service_name="test-service",
        transport_config=MockConfig(),
        exporter=span_exporter,
    )
    tracer = get_tracer("test-service")
    try:
        yield tracer
    finally:
        _reset_tracing_state()
