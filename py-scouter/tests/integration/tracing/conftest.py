from scouter.tracing import (
    BatchConfig,
    HttpSpanExporter,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
import pytest


@pytest.fixture(scope="session")
def span_exporter():
    """Create a fresh http span exporter for each test.
    This should write to the otel collector
    """
    return HttpSpanExporter()


@pytest.fixture(scope="session")
def tracer(span_exporter):
    """Initialize tracer with test exporter for each test."""
    init_tracer(
        name="test-service",
        exporter=span_exporter,
        batch_config=BatchConfig(scheduled_delay_ms=200),
    )
    yield get_tracer("test-tracer")

    shutdown_tracer()
