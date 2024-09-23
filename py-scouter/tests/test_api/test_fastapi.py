from scouter.integrations.fastapi import ScouterRouter
from fastapi.testclient import TestClient


def test_route(client: TestClient):
    response = client.get("/test")
    assert response.status_code == 200
    print(response.json())
    a
