import time

from opentelemetry import trace
from scouter.mock import MockConfig, ScouterTestServer
from scouter.tracing import TracerProvider

provider = TracerProvider(transport_config=MockConfig())
trace.set_tracer_provider(provider)
tracer = trace.get_tracer("test.scouter.span.exporter")


def test_scouter_span_exporter():
    with ScouterTestServer() as _server:
        with tracer.start_as_current_span("test-span-scouter-exporter") as span:
            span.set_attribute("test.attribute", "value1")
            time.sleep(0.1)  # Simulate work

        with tracer.start_as_current_span("test-span-scouter-exporter-2") as span:
            span.set_attribute("test.attribute", "value1")
            time.sleep(0.1)  # Simulate work
