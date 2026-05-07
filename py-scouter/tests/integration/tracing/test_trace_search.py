"""Integration tests for trace search DSL."""

import pytest
from scouter.client import ScouterClient
from scouter.tracing import TraceFilters


def test_from_query_basic_phrase() -> None:
    filters = TraceFilters.from_query("kafka")
    assert filters.clause is not None


def test_from_query_grouping_and_top_level_routing() -> None:
    filters = TraceFilters.from_query(
        "start_time:2026-05-05T00:00:00Z entity_uid:abc-123 (kafka OR redis) AND status:error"
    )
    assert filters.start_time is not None
    assert filters.entity_uid == "abc-123"
    assert filters.clause is not None


def test_from_query_invalid_raises() -> None:
    with pytest.raises(Exception):
        TraceFilters.from_query("(start_time:2026-05-05T00:00:00Z kafka)")


def test_client_exposes_search_traces() -> None:
    assert hasattr(ScouterClient, "search_traces")
