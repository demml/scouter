from scouter.integrations.fastapi import ScouterRouter
from fastapi.testclient import TestClient
from tests.conftest import TestDriftRecord
from typing import Any, Dict, List, Tuple


def test_route(client: TestClient):
    response = client.get("/test")
    assert response.status_code == 200
    assert response.json() == {"message": "success"}


def test_ingestion(client_insert: Tuple[TestClient, List[Dict[str, Any]]]):
    client, records = client_insert

    response = client.post(
        "/scouter/drift",
        json=TestDriftRecord(
            name="test",
            repository="test",
            version="test",
            feature="test",
            value=1.0,
        ).model_dump(),
    )
    assert response.status_code == 200
