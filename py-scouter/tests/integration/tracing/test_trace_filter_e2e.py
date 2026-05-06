"""End-to-end coverage for trace search filters over persisted spans."""

import time
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any

from scouter.client import ScouterClient

from .conftest import (
    INSTRUMENTOR_HTTP_SERVICE,
    _wait_for_export,
)


def _trace_id(value: Any) -> str:
    if isinstance(value, int):
        return f"{value:032x}"
    return str(value)


def _scouter_client(timeout_seconds: float = 10.0) -> ScouterClient:
    deadline = time.monotonic() + timeout_seconds
    last_error: Exception | None = None

    while time.monotonic() < deadline:
        try:
            return ScouterClient()
        except Exception as exc:  # pylint: disable=broad-exception-caught
            last_error = exc
            time.sleep(0.5)

    raise RuntimeError("ScouterClient was not ready") from last_error


def _attribute_map(span: Any) -> dict[str, str]:
    return {attr.key: str(attr.value) for attr in span.attributes}


def _trace_has_attributes(
    client: ScouterClient,
    trace_id: str,
    expected: dict[str, str],
) -> bool:
    response = client.get_trace_spans(trace_id)
    return any(
        all(_attribute_map(span).get(key) == value for key, value in expected.items())
        for span in response.spans
    )


def _wait_for_trace_ids(
    client: ScouterClient,
    query: str,
    start_time: datetime,
    end_time: datetime,
    *,
    minimum_count: int = 1,
    timeout_seconds: float = 12.0,
) -> set[str]:
    deadline = time.monotonic() + timeout_seconds
    last_ids: set[str] = set()

    while time.monotonic() < deadline:
        response = client.search_traces(query, start_time, end_time, 20)
        last_ids = {item.trace_id for item in response.items}
        if len(last_ids) >= minimum_count:
            return last_ids
        time.sleep(0.5)

    return last_ids


def test_trace_filter_planner_python_e2e(setup_instrumentor_http) -> None:
    tracer, _server, _instrumentor = setup_instrumentor_http
    token = uuid.uuid4().hex
    start_time = datetime.now(timezone.utc) - timedelta(minutes=5)

    with tracer.start_as_current_span("filter-e2e-kafka") as span:
        span.set_attribute("e2e_filter_id", token)
        span.set_attribute("component", "kafka")
        kafka_trace_id = _trace_id(span.trace_id)

    with tracer.start_as_current_span("filter-e2e-redis") as span:
        span.set_attribute("e2e_filter_id", token)
        span.set_attribute("component", "redis")
        redis_trace_id = _trace_id(span.trace_id)

    with tracer.start_as_current_span("filter-e2e-quoted-service") as span:
        span.set_attribute("e2e_filter_id", token)
        span.set_attribute("service", "checkout")
        quoted_service_trace_id = _trace_id(span.trace_id)

    _wait_for_export()

    client = _scouter_client()
    end_time = datetime.now(timezone.utc) + timedelta(minutes=5)

    service_ids = _wait_for_trace_ids(
        client,
        f"service:{INSTRUMENTOR_HTTP_SERVICE} AND e2e_filter_id:{token}",
        start_time,
        end_time,
        minimum_count=3,
    )

    assert {kafka_trace_id, redis_trace_id, quoted_service_trace_id}.issubset(
        service_ids
    )

    mixed_ids = _wait_for_trace_ids(
        client,
        f"service:no-such-service OR (e2e_filter_id:{token} AND component:kafka)",
        start_time,
        end_time,
    )

    assert kafka_trace_id in mixed_ids
    assert redis_trace_id not in mixed_ids
    assert _trace_has_attributes(
        client,
        kafka_trace_id,
        {"e2e_filter_id": token, "component": "kafka"},
    )

    quoted_attr_ids = _wait_for_trace_ids(
        client,
        f'"service":"checkout" AND e2e_filter_id:{token}',
        start_time,
        end_time,
    )
    builtin_service_ids = _wait_for_trace_ids(
        client,
        f"service:checkout AND e2e_filter_id:{token}",
        start_time,
        end_time,
        minimum_count=0,
        timeout_seconds=1.0,
    )

    assert quoted_service_trace_id in quoted_attr_ids
    assert quoted_service_trace_id not in builtin_service_ids
    assert _trace_has_attributes(
        client,
        quoted_service_trace_id,
        {"e2e_filter_id": token, "service": "checkout"},
    )
