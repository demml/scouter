import pytest
from pydantic import BaseModel
from scouter.tracing import TestSpanExporter, get_tracer, init_tracer
from scouter.mock import ScouterTestServer


class ChatInput(BaseModel):
    message: str
    user_id: int


class EventData(BaseModel):
    foo: str
    bar: int
    baz: float


@pytest.fixture(scope="session")
def http_scouter_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture(scope="session")
def span_exporter():
    """Create a fresh test span exporter for each test."""
    return TestSpanExporter()


@pytest.fixture(scope="session")
def tracer(http_scouter_server, span_exporter):
    """Initialize tracer with test exporter for each test."""
    init_tracer(name="test-service", exporter=span_exporter)
    return get_tracer("test-tracer")
