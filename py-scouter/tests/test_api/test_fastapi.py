from fastapi.testclient import TestClient
from tests.conftest import PredictRequest
from unittest import mock


def test_route(client: TestClient):
    response = client.get("/test")
    assert response.status_code == 200
    assert response.json() == {"message": "success"}


@mock.patch("scouter.integrations.http.HTTPProducer.request")
def test_insert_http(
    mock_request: mock.MagicMock,
    client_insert: TestClient,
):
    mock_request.return_value = {"message": "success"}

    for i in range(26):
        response = client_insert.post(
            "/predict",
            json=PredictRequest(
                feature_0=1.0,
                feature_1=1.0,
                feature_2=1.0,
            ).model_dump(),
        )
    assert response.status_code == 200

    # called once for each feature after sampling
    assert mock_request.call_count == 3
