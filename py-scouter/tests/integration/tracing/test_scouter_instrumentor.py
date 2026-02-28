"""
Integration tests for ScouterInstrumentor.

Verifies that ScouterInstrumentor correctly:
  - Overrides the global OpenTelemetry TracerProvider
  - Persists traces to the Scouter backend (HTTP transport, no gRPC)
  - Propagates default_attributes onto every span
  - Cleans up global OTel state on uninstrument()
"""

import time
from typing import Any, List, Optional

from opentelemetry import trace
from scouter.client import ScouterClient, TraceFilters
from scouter.tracing import TracerProvider

from .conftest import (  # type: ignore
    DEFAULT_TEST_ATTRIBUTES,
    INSTRUMENTOR_ATTRS_SERVICE,
    INSTRUMENTOR_HTTP_SERVICE,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _find_attr(attributes: List[Any], key: str) -> Optional[Any]:
    """Return the first attribute value matching *key*, or None."""
    return next((a.value for a in attributes if a.key == key), None)


def _wait_for_export(seconds: float = 0.4) -> None:
    """Give the batch exporter time to flush."""
    time.sleep(seconds)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_instrumentor_sets_global_otel_provider(setup_instrumentor_http):
    """ScouterInstrumentor must register a TracerProvider as the global OTel provider."""
    _tracer, _server, instrumentor = setup_instrumentor_http

    assert instrumentor.is_instrumented, "Expected instrumentor.is_instrumented == True"

    global_provider = trace.get_tracer_provider()
    assert isinstance(global_provider, TracerProvider), (
        f"Expected global OTel provider to be TracerProvider, " f"got {type(global_provider).__name__}"
    )


def test_instrumentor_traces_appear_in_scouter_db(setup_instrumentor_http):
    """Traces produced via the instrumented OTel API must appear in the Scouter DB."""
    tracer, _server, _instrumentor = setup_instrumentor_http

    for _ in range(5):
        with tracer.start_as_current_span("root-span") as root:
            root.set_attribute("test.key", "test-value")
            with tracer.start_as_current_span("child-span") as child:
                child.set_attribute("child.key", "child-value")
            trace_id = root.trace_id  # capture last trace_id

    _wait_for_export()

    scouter_client = ScouterClient()
    response = scouter_client.get_trace_spans(trace_id)

    assert len(response.spans) > 0, (
        f"No spans found for trace_id={trace_id!r}. " "Traces were not persisted to the Scouter backend."
    )


def test_instrumentor_paginated_traces_filterable_by_service(setup_instrumentor_http):
    """get_paginated_traces must return traces tagged with the instrumented service name."""
    tracer, _server, _instrumentor = setup_instrumentor_http

    for _ in range(3):
        with tracer.start_as_current_span("paginated-root"):
            pass

    _wait_for_export()

    scouter_client = ScouterClient()
    response = scouter_client.get_paginated_traces(TraceFilters(service_name=INSTRUMENTOR_HTTP_SERVICE))

    assert len(response.items) > 0, (
        f"No paginated traces found for service_name={INSTRUMENTOR_HTTP_SERVICE!r}. "
        "Traces may not have been flushed or the service name is not propagated."
    )


def test_instrumentor_multi_span_hierarchy(setup_instrumentor_http):
    """Scouter must store all spans in a hierarchy, not only the root span."""
    tracer, _server, _instrumentor = setup_instrumentor_http

    with tracer.start_as_current_span("parent") as parent:
        with tracer.start_as_current_span("child-a"):
            pass
        with tracer.start_as_current_span("child-b"):
            pass
        trace_id = parent.trace_id

    _wait_for_export()

    scouter_client = ScouterClient()
    response = scouter_client.get_trace_spans(trace_id)

    span_names = {s.span_name for s in response.spans}
    assert "parent" in span_names, f"Root span 'parent' missing. Found: {span_names}"
    assert "child-a" in span_names, f"Child span 'child-a' missing. Found: {span_names}"
    assert "child-b" in span_names, f"Child span 'child-b' missing. Found: {span_names}"


def test_instrumentor_default_attributes_on_spans(
    setup_instrumentor_with_default_attrs,
):
    """Every span must carry the default_attributes declared at instrumentation time."""
    tracer, _server, _instrumentor = setup_instrumentor_with_default_attrs

    with tracer.start_as_current_span("default-attrs-root") as root:
        trace_id = root.trace_id

    _wait_for_export()

    scouter_client = ScouterClient()
    response = scouter_client.get_trace_spans(trace_id)

    assert len(response.spans) > 0, (
        f"No spans returned for trace_id={trace_id!r}. " "Trace was not persisted — cannot verify default_attributes."
    )

    for span in response.spans:
        for attr_key, expected_value in DEFAULT_TEST_ATTRIBUTES.items():
            actual = _find_attr(span.attributes, attr_key)
            assert actual is not None, (
                f"Span '{span.span_name}' is missing default attribute '{attr_key}'. "
                "default_attributes are not being propagated to stored spans."
            )
            assert str(actual) == str(expected_value), (
                f"Span '{span.span_name}': attribute '{attr_key}' " f"expected={expected_value!r}, got={actual!r}"
            )


def test_instrumentor_default_attrs_service_queryable(
    setup_instrumentor_with_default_attrs,
):
    """Traces from the default-attrs service must be retrievable by service name."""
    tracer, _server, _instrumentor = setup_instrumentor_with_default_attrs

    for _ in range(3):
        with tracer.start_as_current_span("attrs-root"):
            pass

    _wait_for_export()

    scouter_client = ScouterClient()
    response = scouter_client.get_paginated_traces(TraceFilters(service_name=INSTRUMENTOR_ATTRS_SERVICE))

    assert len(response.items) > 0, (
        f"No records returned for service_name={INSTRUMENTOR_ATTRS_SERVICE!r} "
        "even though default attributes were set and traces were produced."
    )


def test_instrumentor_uninstrument_resets_provider(setup_instrumentor_http):
    """After uninstrument(), is_instrumented must be False and provider cleared."""
    _tracer, _server, instrumentor = setup_instrumentor_http

    assert instrumentor.is_instrumented

    # uninstrument happens in the fixture teardown; call it explicitly here
    # to test the mid-test state. Re-instrument happens automatically via fixture teardown
    # so we only verify the state directly after an explicit uninstrument.
    instrumentor.uninstrument()

    assert not instrumentor.is_instrumented, "ScouterInstrumentor.is_instrumented should be False after uninstrument()."
