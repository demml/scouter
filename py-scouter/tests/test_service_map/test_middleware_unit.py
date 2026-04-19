"""Unit tests for ServiceMapMiddleware.__call__.

Tests the middleware logic in isolation — no Bifrost server or gRPC needed.
Bifrost.insert is patched to capture emitted records.
"""
from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock, patch

import pytest


# ---------------------------------------------------------------------------
# ASGI helpers
# ---------------------------------------------------------------------------


def _http_scope(
    path: str = "/health",
    method: str = "GET",
    headers: list[tuple[bytes, bytes]] | None = None,
    scope_type: str = "http",
) -> dict[str, Any]:
    return {
        "type": scope_type,
        "method": method,
        "path": path,
        "headers": headers or [],
    }


def _make_receive(body: bytes = b"", content_type: str = "application/json") -> Any:
    called = False

    async def receive() -> dict[str, Any]:
        nonlocal called
        if not called:
            called = True
            return {"type": "http.request", "body": body, "more_body": False}
        return {"type": "http.disconnect"}

    return receive


async def _noop_send(message: Any) -> None:
    pass


async def _status_send(status: int):
    async def send(message: Any) -> None:
        pass

    return send


def _make_send(responses: list[dict[str, Any]]):
    idx = 0

    async def send(message: Any) -> None:
        nonlocal idx
        responses.append(message)

    return send


# ---------------------------------------------------------------------------
# Middleware factory (bypasses Bifrost.__init__ gRPC setup)
# ---------------------------------------------------------------------------


def _make_middleware(
    inner_app: Any,
    service_name: str = "test-svc",
    capture_schema: bool = False,
    max_body_bytes: int = 16_777_216,
) -> Any:
    from scouter.service_map.middleware import ServiceMapMiddleware

    with patch("scouter.service_map.middleware.Bifrost") as mock_bifrost_cls:
        mock_bifrost = MagicMock()
        mock_bifrost_cls.return_value = mock_bifrost
        mw = ServiceMapMiddleware(
            app=inner_app,
            grpc_address="http://localhost:50051",
            service_name=service_name,
            capture_schema=capture_schema,
            max_body_bytes=max_body_bytes,
        )
        mw._bifrost = mock_bifrost  # keep mock accessible post-construction
    return mw


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_websocket_scope_passes_through_without_record():
    """WebSocket scopes must be forwarded without emitting a ServiceConnectionRecord."""
    received: list[Any] = []

    async def inner(scope, receive, send):
        received.append(scope["type"])

    mw = _make_middleware(inner)

    scope = _http_scope(scope_type="websocket")
    await mw(scope, _make_receive(), _noop_send)

    assert received == ["websocket"], "inner app should have been called"
    mw._bifrost.insert.assert_not_called()


@pytest.mark.asyncio
async def test_lifespan_scope_passes_through_without_record():
    """Non-http/websocket scopes (e.g. lifespan) must be forwarded without recording."""
    received: list[Any] = []

    async def inner(scope, receive, send):
        received.append(scope["type"])

    mw = _make_middleware(inner)

    scope = _http_scope(scope_type="lifespan")
    await mw(scope, _make_receive(), _noop_send)

    assert received == ["lifespan"]
    mw._bifrost.insert.assert_not_called()


@pytest.mark.asyncio
async def test_http_request_emits_record():
    """A plain GET request produces exactly one ServiceConnectionRecord."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    mw = _make_middleware(inner, service_name="my-api")

    scope = _http_scope(path="/health", method="GET")
    await mw(scope, _make_receive(), _noop_send)

    mw._bifrost.insert.assert_called_once()
    record = mw._bifrost.insert.call_args[0][0]
    assert record.destination_service == "my-api"
    assert record.endpoint == "/health"
    assert record.method == "GET"
    assert record.status_code == 200
    assert record.error is False


@pytest.mark.asyncio
async def test_500_sets_error_flag():
    """5xx status codes set error=True on the emitted record."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 500})

    mw = _make_middleware(inner)

    scope = _http_scope(path="/boom", method="GET")
    await mw(scope, _make_receive(), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.status_code == 500
    assert record.error is True


@pytest.mark.asyncio
async def test_source_service_from_header():
    """x-scouter-service header is used as source_service."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    headers = [(b"x-scouter-service", b"upstream-caller")]
    mw = _make_middleware(inner)

    scope = _http_scope(headers=headers)
    await mw(scope, _make_receive(), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.source_service == "upstream-caller"


@pytest.mark.asyncio
async def test_unknown_source_when_no_header():
    """Missing x-scouter-service header defaults source_service to 'unknown'."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    mw = _make_middleware(inner)
    await mw(_http_scope(), _make_receive(), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.source_service == "unknown"


@pytest.mark.asyncio
async def test_malformed_content_length_does_not_crash():
    """Non-numeric Content-Length header with capture_schema=True must not raise."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    headers = [
        (b"content-type", b"application/json"),
        (b"content-length", b"not-a-number"),
    ]
    mw = _make_middleware(inner, capture_schema=True)

    scope = _http_scope(headers=headers)
    body = json.dumps({"x": 1}).encode()
    await mw(scope, _make_receive(body=body), _noop_send)

    # Should still emit a record (schema capture falls back gracefully)
    mw._bifrost.insert.assert_called_once()


@pytest.mark.asyncio
async def test_capture_schema_enabled_records_field_types():
    """With capture_schema=True, JSON body field types are stored in request_schema."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    headers = [
        (b"content-type", b"application/json"),
        (b"content-length", b"50"),
    ]
    payload = {"user": "alice", "score": 0.9}
    body = json.dumps(payload).encode()

    mw = _make_middleware(inner, capture_schema=True)
    scope = _http_scope(method="POST", headers=headers)
    await mw(scope, _make_receive(body=body), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.request_schema is not None
    schema = json.loads(record.request_schema)
    assert schema["user"] == "string"
    assert schema["score"] == "number"


@pytest.mark.asyncio
async def test_capture_schema_disabled_by_default():
    """With capture_schema=False (default), request_schema is None even for JSON bodies."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    headers = [
        (b"content-type", b"application/json"),
        (b"content-length", b"30"),
    ]
    body = json.dumps({"x": 1}).encode()

    mw = _make_middleware(inner, capture_schema=False)
    scope = _http_scope(method="POST", headers=headers)
    await mw(scope, _make_receive(body=body), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.request_schema is None


@pytest.mark.asyncio
async def test_path_ids_are_normalized():
    """Numeric path segments are replaced with {id} before recording."""

    async def inner(scope, receive, send):
        await send({"type": "http.response.start", "status": 200})

    mw = _make_middleware(inner)
    scope = _http_scope(path="/models/99999/predict")
    await mw(scope, _make_receive(), _noop_send)

    record = mw._bifrost.insert.call_args[0][0]
    assert record.endpoint == "/models/{id}/predict"
