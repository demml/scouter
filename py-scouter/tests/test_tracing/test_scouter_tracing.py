import time

from opentelemetry import trace
from opentelemetry.sdk import trace as trace_sdk
from opentelemetry.sdk.trace.export import SimpleSpanProcessor
from scouter.mock import ScouterTestServer
from scouter.tracing import ScouterSpanExporter

provider = trace_sdk.TracerProvider()
provider.add_span_processor(SimpleSpanProcessor(ScouterSpanExporter()))
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
