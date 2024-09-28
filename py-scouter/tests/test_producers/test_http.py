from scouter import HTTPConfig, HTTPProducer, SpcDriftServerRecord


def test_http_config():
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    assert config.server_url == "http://localhost:8000"
    assert config.username == "test-username"
    assert config.password == "test-password"


def test_http_producer(mock_httpx_producer):
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = HTTPProducer(config)

    record = SpcDriftServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(record)
