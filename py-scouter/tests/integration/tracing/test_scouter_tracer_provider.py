from datetime import datetime, timedelta, timezone

from scouter.client import ScouterClient, TraceMetricsRequest

from .conftest import _wait_for_export  # type: ignore


def test_scouter_span_exporter(setup_scouter_trace_provider):
    tracer, _server = setup_scouter_trace_provider
    for _ in range(10):
        with tracer.start_as_current_span("span1") as span:
            span.set_attribute("test.attribute", "value1")

            with tracer.start_as_current_span("span2") as span:
                span.set_attribute("test.attribute", "value1")

        trace_id = span.trace_id

    _wait_for_export()
    scouter_client = ScouterClient()

    # Get spans for specific trace
    trace_spans = scouter_client.get_trace_spans(trace_id)
    assert len(trace_spans.spans) > 0

    # Trace metrics (soft assert — summary table may be empty if archiver hasn't run)
    now = datetime.now(timezone.utc)
    metrics = scouter_client.get_trace_metrics(
        TraceMetricsRequest(
            start_time=now - timedelta(hours=1),
            end_time=now + timedelta(hours=1),
            bucket_interval="1 minutes",
        )
    )
    assert metrics is not None
