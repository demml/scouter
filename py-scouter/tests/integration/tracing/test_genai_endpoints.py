"""Integration tests for GenAI semantic-convention endpoints.

These tests start a real ScouterTestServer, seed GenAI spans via the scouter
tracer (with gen_ai.* attributes), wait for the Delta Lake flush, then verify
every new HTTP endpoint returns 200 with the correct response envelope shape.

Tests are intentionally endpoint-focused: we assert shape not exact values so
the suite stays stable across flush timing variations.
"""

import os
import time
from datetime import datetime, timedelta, timezone

import pytest
import requests
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

FLUSH_WAIT_SECS = 8  # buffer flush (1s) + Delta write + refresh (1s) + margin
SERVICE_NAME = "genai-test-service"
CONVERSATION_ID = "conv-genai-test-001"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _server_url() -> str:
    """Return the scouter server base URL from the env var set by ScouterTestServer."""
    return os.environ.get("SCOUTER_SERVER_URI", "http://localhost:3000")


def _get_auth_token(server_url: str) -> str:
    """Obtain a JWT bearer token via the scouter auth endpoint."""
    response = requests.get(
        f"{server_url}/scouter/auth/login",
        headers={"Username": "admin", "Password": "admin"},
        timeout=10,
    )
    response.raise_for_status()
    return response.json()["token"]


def _make_session(server_url: str) -> requests.Session:
    """Return a requests.Session pre-loaded with a valid bearer token."""
    token = _get_auth_token(server_url)
    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})
    return session


def _time_range() -> dict:
    """Return a start/end time range covering the last 30 minutes as RFC 3339 strings."""
    now = datetime.now(timezone.utc)
    return {
        "start_time": (now - timedelta(minutes=30)).isoformat(),
        "end_time": now.isoformat(),
    }


def _seed_genai_spans(tracer) -> None:
    """Emit several spans with gen_ai.* attributes so the Delta table has data."""
    gen_ai_attrs = {
        "gen_ai.operation.name": "chat",
        "gen_ai.provider.name": "anthropic",
        "gen_ai.request.model": "claude-3-5-sonnet",
        "gen_ai.usage.input_tokens": "150",
        "gen_ai.usage.output_tokens": "75",
        "gen_ai.conversation.id": CONVERSATION_ID,
    }

    for _ in range(3):
        with tracer.start_as_current_span("llm_call") as span:
            for key, value in gen_ai_attrs.items():
                span.set_attribute(key, value)
            time.sleep(0.01)

    # One tool-call span
    tool_attrs = {
        "gen_ai.operation.name": "execute_tool",
        "gen_ai.provider.name": "anthropic",
        "gen_ai.request.model": "claude-3-5-sonnet",
        "gen_ai.tool.name": "web_search",
        "gen_ai.tool.type": "function",
        "gen_ai.conversation.id": CONVERSATION_ID,
    }
    with tracer.start_as_current_span("tool_call") as span:
        for key, value in tool_attrs.items():
            span.set_attribute(key, value)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture()
def genai_server_with_data():
    """Start ScouterTestServer, seed GenAI spans, wait for flush, yield session."""
    with ScouterTestServer() as _server:
        init_tracer(
            service_name=SERVICE_NAME,
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
        )
        tracer = get_tracer(SERVICE_NAME)

        _seed_genai_spans(tracer)

        # Flush the batch exporter and wait for Delta write
        time.sleep(FLUSH_WAIT_SECS)

        url = _server_url()
        session = _make_session(url)

        yield url, session

        shutdown_tracer()


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_genai_token_metrics_endpoint(genai_server_with_data):
    """POST /scouter/genai/metrics/tokens returns 200 with a 'buckets' list."""
    url, session = genai_server_with_data
    body = {
        "service_name": SERVICE_NAME,
        **_time_range(),
        "bucket_interval": "hour",
    }
    resp = session.post(f"{url}/scouter/genai/metrics/tokens", json=body, timeout=15)
    assert resp.status_code == 200, resp.text
    data = resp.json()
    assert "buckets" in data
    assert isinstance(data["buckets"], list)


def test_genai_model_usage_endpoint(genai_server_with_data):
    """POST /scouter/genai/metrics/models returns 200 with a 'models' list."""
    url, session = genai_server_with_data
    body = {
        "service_name": SERVICE_NAME,
        **_time_range(),
    }
    resp = session.post(f"{url}/scouter/genai/metrics/models", json=body, timeout=15)
    assert resp.status_code == 200, resp.text
    data = resp.json()
    assert "models" in data
    assert isinstance(data["models"], list)


def test_genai_operation_breakdown_endpoint(genai_server_with_data):
    """POST /scouter/genai/metrics/operations returns 200 with an 'operations' list."""
    url, session = genai_server_with_data
    body = {
        "service_name": SERVICE_NAME,
        **_time_range(),
    }
    resp = session.post(f"{url}/scouter/genai/metrics/operations", json=body, timeout=15)
    assert resp.status_code == 200, resp.text
    data = resp.json()
    assert "operations" in data
    assert isinstance(data["operations"], list)


def test_genai_spans_explorer_endpoint(genai_server_with_data):
    """POST /scouter/genai/spans returns 200 with a 'spans' list."""
    url, session = genai_server_with_data
    body = {
        "service_name": SERVICE_NAME,
        **_time_range(),
        "limit": 50,
    }
    resp = session.post(f"{url}/scouter/genai/spans", json=body, timeout=15)
    assert resp.status_code == 200, resp.text
    data = resp.json()
    assert "spans" in data
    assert isinstance(data["spans"], list)


def test_genai_conversation_endpoint(genai_server_with_data):
    """GET /scouter/genai/conversation/{id} returns 200 with a 'spans' list."""
    url, session = genai_server_with_data
    time_range = _time_range()
    params = {
        "start_time": time_range["start_time"],
        "end_time": time_range["end_time"],
    }
    resp = session.get(
        f"{url}/scouter/genai/conversation/{CONVERSATION_ID}",
        params=params,
        timeout=15,
    )
    assert resp.status_code == 200, resp.text
    data = resp.json()
    assert "spans" in data
    assert isinstance(data["spans"], list)
