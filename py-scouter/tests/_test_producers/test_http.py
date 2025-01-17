from scouter import (
    CustomMetricServerRecord,
    HTTPConfig,
    HTTPProducer,
    PsiServerRecord,
    RecordType,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


def test_http_config():
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    assert config.server_url == "http://localhost:8000"
    assert config.username == "test-username"
    assert config.password == "test-password"


def test_http_producer_spc(mock_httpx_producer):
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = HTTPProducer(config)

    record = SpcServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(
        records=ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )


def test_http_producer_psi(mock_httpx_producer):
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = HTTPProducer(config)

    record = PsiServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        bin_id="test",
        bin_count=1,
    )

    producer.publish(
        records=ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )


def test_http_producer_custom(mock_httpx_producer):
    config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = HTTPProducer(config)

    record = CustomMetricServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        metric="metric",
        value=0.1,
    )

    producer.publish(
        records=ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )
