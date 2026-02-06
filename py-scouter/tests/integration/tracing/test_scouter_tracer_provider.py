import time

from scouter.client import ScouterClient


def test_scouter_span_exporter(setup_scouter_trace_provider):
    tracer, _server = setup_scouter_trace_provider
    for _ in range(10):
        with tracer.start_as_current_span("span1") as span:
            span.set_attribute("test.attribute", "value1")

            with tracer.start_as_current_span("span2") as span:
                span.set_attribute("test.attribute", "value1")

        trace_id = span.trace_id

    time.sleep(0.3)
    scouter_client = ScouterClient()
    trace_spans = scouter_client.get_trace_spans(trace_id)

    assert len(trace_spans.spans) > 0
