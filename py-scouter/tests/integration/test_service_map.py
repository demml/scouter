"""Integration tests for ServiceMapMiddleware.

Validates the full lifecycle: middleware captures connection events →
records flushed to Bifrost → queryable via DatasetClient SQL.

Uses BifrostTestServer — no Docker required.
"""

import time

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient
from scouter.bifrost import DatasetClient, TableConfig
from scouter.mock import BifrostTestServer
from scouter.service_map import (
    SERVICE_MAP_CATALOG,
    SERVICE_MAP_SCHEMA,
    SERVICE_MAP_TABLE,
    ServiceConnectionRecord,
    ServiceMapMiddleware,
)
from scouter.transport import GrpcConfig

FLUSH_WAIT_SECS = 8
GRPC_URI = "http://localhost:50051"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_app(
    service_name: str = "test-service",
    tags: dict | None = None,
    capture_schema: bool = False,
    max_body_bytes: int = 16_777_216,
    batch_size: int = 1,
    flush_interval_secs: int = 1,
) -> FastAPI:
    app = FastAPI()

    app.add_middleware(
        ServiceMapMiddleware,
        grpc_address=GRPC_URI,
        service_name=service_name,
        tags=tags,
        capture_schema=capture_schema,
        max_body_bytes=max_body_bytes,
        batch_size=batch_size,
        flush_interval_secs=flush_interval_secs,
    )

    @app.get("/health")
    def health():
        return {"status": "ok"}

    @app.get("/models/{model_id}/predict")
    def predict(model_id: str):
        return {"model_id": model_id, "score": 0.95}

    @app.post("/infer")
    def infer(body: dict):
        return {"result": "ok"}

    @app.get("/error")
    def error():
        from fastapi import HTTPException

        raise HTTPException(status_code=500, detail="boom")

    return app


def _make_client(app: FastAPI) -> TestClient:
    return TestClient(app, raise_server_exceptions=False)


def _bifrost_client() -> DatasetClient:
    config = TableConfig(
        model=ServiceConnectionRecord,
        catalog=SERVICE_MAP_CATALOG,
        schema_name=SERVICE_MAP_SCHEMA,
        table=SERVICE_MAP_TABLE,
    )
    return DatasetClient(transport=GrpcConfig(server_uri=GRPC_URI), table_config=config)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture()
def bifrost_server():
    with BifrostTestServer() as server:
        yield server


# ---------------------------------------------------------------------------
# Unit-level helper tests (no server needed)
# ---------------------------------------------------------------------------


def test_endpoint_normalization():
    from scouter.service_map.middleware import _normalize_endpoint

    assert _normalize_endpoint("/users/12345/orders") == "/users/{id}/orders"
    assert _normalize_endpoint("/models/abc123de-f456-7890-abcd-ef1234567890/predict") == "/models/{id}/predict"
    assert _normalize_endpoint("/health") == "/health"
    assert _normalize_endpoint("/v1/api/42/items/99") == "/v1/api/{id}/items/{id}"


def test_trace_id_extraction():
    from scouter.service_map.middleware import _extract_trace_id

    valid = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    assert _extract_trace_id(valid) == "4bf92f3577b34da6a3ce929d0e0e4736"

    assert _extract_trace_id("invalid") is None
    assert _extract_trace_id("") is None


def test_schema_inference():
    import json

    from scouter.service_map.middleware import _infer_schema

    body = json.dumps({"user_id": "abc", "score": 0.9, "count": 5, "active": True}).encode()
    result = _infer_schema(body)
    assert result is not None
    schema = json.loads(result)
    assert schema["user_id"] == "string"
    assert schema["score"] == "number"
    assert schema["count"] == "number"
    assert schema["active"] == "boolean"

    assert _infer_schema(b"not json") is None
    assert _infer_schema(b"[1, 2, 3]") is None  # list, not object


def test_schema_inference_nested_types():
    import json

    from scouter.service_map.middleware import _infer_schema

    body = json.dumps({"meta": {}, "items": [], "label": None}).encode()
    result = _infer_schema(body)
    assert result is not None
    schema = json.loads(result)
    assert schema["meta"] == "object"
    assert schema["items"] == "array"
    assert schema["label"] == "null"


def test_service_connection_record_fields():
    from datetime import datetime, timezone

    record = ServiceConnectionRecord(
        source_service="caller-a",
        destination_service="service-b",
        endpoint="/api/predict",
        method="POST",
        status_code=200,
        latency_ms=12.5,
        timestamp=datetime.now(timezone.utc),
        trace_id="abc123",
        error=False,
        request_schema=None,
    )
    assert record.source_service == "caller-a"
    assert record.destination_service == "service-b"
    assert not record.error


# ---------------------------------------------------------------------------
# FastAPI middleware integration tests
# ---------------------------------------------------------------------------


def test_middleware_captures_basic_request(bifrost_server) -> None:
    """GET request → record lands in Bifrost with correct service identity."""
    client = _make_client(_make_app(service_name="recommendation-api"))
    client.get("/health", headers={"x-scouter-service": "load-balancer"})
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    hit = next(
        (r for r in records if r.endpoint == "/health" and r.source_service == "load-balancer"),
        None,
    )
    assert hit is not None
    assert hit.destination_service == "recommendation-api"
    assert hit.method == "GET"
    assert hit.status_code == 200
    assert not hit.error


def test_middleware_normalizes_path_ids(bifrost_server) -> None:
    """URL path IDs are stripped to ``{id}`` to prevent cardinality explosion."""
    app = _make_app(service_name="test-normalize-ids")
    client = _make_client(app)

    client.get("/models/12345/predict")
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    own = [r for r in records if r.destination_service == "test-normalize-ids"]
    assert any(r.endpoint == "/models/{id}/predict" for r in own)


def test_middleware_captures_error_status(bifrost_server) -> None:
    """5xx responses set ``error=True`` on the record."""
    app = _make_app(service_name="test-error-status")
    client = _make_client(app)

    client.get("/error")
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    error_records = [r for r in records if r.destination_service == "test-error-status" and r.endpoint == "/error"]

    assert len(error_records) >= 1
    assert all(r.error for r in error_records)
    assert all(r.status_code == 500 for r in error_records)


def test_middleware_extracts_traceparent(bifrost_server) -> None:
    """Trace ID is parsed from the W3C ``traceparent`` header."""
    app = _make_app(service_name="test-traceparent")
    client = _make_client(app)

    trace_id = "4bf92f3577b34da6a3ce929d0e0e4736"
    traceparent = f"00-{trace_id}-00f067aa0ba902b7-01"
    client.get("/health", headers={"traceparent": traceparent})
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    own = [r for r in records if r.destination_service == "test-traceparent"]

    assert any(r.trace_id == trace_id for r in own)


def test_middleware_unknown_source_when_no_header(bifrost_server) -> None:
    """Requests without ``x-scouter-service`` header get source_service='unknown'."""
    app = _make_app(service_name="test-unknown-source")
    client = _make_client(app)

    client.get("/health")
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    own = [r for r in records if r.destination_service == "test-unknown-source"]
    assert any(r.source_service == "unknown" for r in own)


def test_middleware_tags(bifrost_server) -> None:
    """Tags passed at init are stamped on every record as a JSON string."""
    import json

    app = _make_app(service_name="tagged-svc", tags={"env": "prod", "region": "us-east-1"})
    client = _make_client(app)

    client.get("/health")
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    tagged = [r for r in records if r.destination_service == "tagged-svc" and r.tags is not None]

    assert len(tagged) >= 1
    parsed = json.loads(tagged[0].tags)  # type: ignore[arg-type]
    assert parsed["env"] == "prod"
    assert parsed["region"] == "us-east-1"


def test_middleware_schema_capture(bifrost_server) -> None:
    """With ``capture_schema=True``, JSON body field names and types are recorded."""
    import json

    app = _make_app(service_name="test-schema-capture", capture_schema=True)
    client = _make_client(app)

    payload = {"user_id": "u1", "model_name": "gpt-4", "threshold": 0.8, "enabled": True}
    client.post(
        "/infer",
        content=json.dumps(payload),
        headers={"content-type": "application/json"},
    )
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    infer_records = [
        r
        for r in records
        if r.destination_service == "test-schema-capture" and r.endpoint == "/infer" and r.request_schema is not None
    ]

    assert len(infer_records) >= 1
    schema = json.loads(infer_records[0].request_schema)  # type: ignore[arg-type]
    assert schema.get("user_id") == "string"
    assert schema.get("model_name") == "string"
    assert schema.get("threshold") == "number"
    assert schema.get("enabled") == "boolean"


def test_middleware_no_schema_capture_by_default(bifrost_server) -> None:
    """With ``capture_schema=False`` (default), request_schema is None even for JSON bodies."""
    import json

    app = _make_app(service_name="test-no-schema", capture_schema=False)
    client = _make_client(app)

    payload = {"user_id": "u1", "score": 0.9}
    client.post(
        "/infer",
        content=json.dumps(payload),
        headers={"content-type": "application/json"},
    )
    time.sleep(FLUSH_WAIT_SECS)

    records: list[ServiceConnectionRecord] = _bifrost_client().read()
    infer_records = [r for r in records if r.destination_service == "test-no-schema" and r.endpoint == "/infer"]

    assert len(infer_records) >= 1
    assert all(r.request_schema is None for r in infer_records)


# ---------------------------------------------------------------------------
# Multi-service topology test
# ---------------------------------------------------------------------------


def test_service_topology_three_services(bifrost_server) -> None:
    """Three services — svc-a, svc-b, svc-c — form a connected graph.

    Simulated call flow:
        svc-a → svc-b  (3 calls)
        svc-b → svc-c  (2 calls)
        svc-a → svc-c  (1 call)

    Each service runs its own middleware (destination_service = itself).
    svc-a is the caller — it never receives requests in this test, so it
    has no middleware; its identity is carried by the x-scouter-service header.

    Assertions:
    - Full graph: svc-a→svc-b, svc-b→svc-c, svc-a→svc-c edges all exist
    - Upstreams of svc-c: both svc-a and svc-b appear as source_service
    - Upstreams of svc-b: only svc-a appears; svc-b is not its own upstream
    """
    app_b = _make_app(service_name="svc-b")
    app_c = _make_app(service_name="svc-c")

    client_b = _make_client(app_b)
    client_c = _make_client(app_c)

    # svc-a → svc-b (svc-b records: source=svc-a, dest=svc-b)
    for _ in range(3):
        client_b.get("/health", headers={"x-scouter-service": "svc-a"})

    # svc-b → svc-c (svc-c records: source=svc-b, dest=svc-c)
    for _ in range(2):
        client_c.get("/health", headers={"x-scouter-service": "svc-b"})

    # svc-a → svc-c directly (svc-c records: source=svc-a, dest=svc-c)
    client_c.get("/health", headers={"x-scouter-service": "svc-a"})

    time.sleep(FLUSH_WAIT_SECS)

    ds_client = DatasetClient(transport=GrpcConfig(server_uri=GRPC_URI))

    # --- Full graph: all edges ---
    all_edges = ds_client.sql(
        f"SELECT source_service, destination_service, COUNT(*) AS total_calls "
        f"FROM {SERVICE_MAP_CATALOG}.{SERVICE_MAP_SCHEMA}.{SERVICE_MAP_TABLE} "
        f"GROUP BY source_service, destination_service "
        f"ORDER BY total_calls DESC"
    ).to_arrow()

    src = all_edges.column("source_service").to_pylist()
    dst = all_edges.column("destination_service").to_pylist()
    edges = set(zip(src, dst))

    assert ("svc-a", "svc-b") in edges, "svc-a → svc-b edge missing"
    assert ("svc-b", "svc-c") in edges, "svc-b → svc-c edge missing"
    assert ("svc-a", "svc-c") in edges, "svc-a → svc-c edge missing"

    # --- Upstreams of svc-c ---
    svc_c_upstreams = ds_client.sql(
        f"SELECT DISTINCT source_service "
        f"FROM {SERVICE_MAP_CATALOG}.{SERVICE_MAP_SCHEMA}.{SERVICE_MAP_TABLE} "
        f"WHERE destination_service = 'svc-c'"
    ).to_arrow()

    upstream_c = set(svc_c_upstreams.column("source_service").to_pylist())
    assert "svc-a" in upstream_c, "svc-a not listed as upstream of svc-c"
    assert "svc-b" in upstream_c, "svc-b not listed as upstream of svc-c"

    # --- Upstreams of svc-b ---
    svc_b_upstreams = ds_client.sql(
        f"SELECT DISTINCT source_service "
        f"FROM {SERVICE_MAP_CATALOG}.{SERVICE_MAP_SCHEMA}.{SERVICE_MAP_TABLE} "
        f"WHERE destination_service = 'svc-b'"
    ).to_arrow()

    upstream_b = set(svc_b_upstreams.column("source_service").to_pylist())
    assert "svc-a" in upstream_b, "svc-a not listed as upstream of svc-b"
    assert "svc-b" not in upstream_b, "svc-b cannot be its own upstream"
