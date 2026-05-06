"""Trace filter planner-facing Python contracts."""

from datetime import datetime, timedelta, timezone

from scouter.client import TraceMetricsRequest
from scouter.tracing import TraceFilters


def test_service_shorthand_parses_to_service_clause() -> None:
    filters = TraceFilters.from_query("service:checkout")

    assert filters.clause == {"op": "service", "value": "checkout"}


def test_quoted_service_key_remains_attribute_clause() -> None:
    filters = TraceFilters.from_query('"service":"checkout"')

    assert filters.clause == {
        "op": "attr",
        "value": {"key": "service", "value": "checkout"},
    }


def test_trace_metrics_request_round_trips_clause() -> None:
    now = datetime.now(timezone.utc)
    request = TraceMetricsRequest.from_query(
        "service:checkout OR component:kafka",
        now - timedelta(hours=1),
        now + timedelta(hours=1),
        "hour",
    )

    # The PyO3 object does not expose fields directly, so construction is the
    # client-side contract: the parsed clause must be accepted by metrics.
    assert request is not None
