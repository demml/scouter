from scouter.client import HTTPConfig
from scouter.queue import (
    CustomMetricServerRecord,
    PsiServerRecord,
    ScouterProducer,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


def test_http_config():
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    assert config.server_uri == "http://localhost:8000"
    assert config.username == "test-username"
    assert config.password == "test-password"


def test_http_producer_spc():
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = ScouterProducer(config)

    record = SpcServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(message=ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_http_producer_psi():
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = ScouterProducer(config)

    record = PsiServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        feature="test",
        bin_id=0,
        bin_count=1,
    )

    producer.publish(message=ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_http_producer_custom():
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="test-username",
        password="test-password",
    )

    producer = ScouterProducer(config)

    record = CustomMetricServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        metric="metric",
        value=0.1,
    )

    producer.publish(message=ServerRecords(records=[ServerRecord(record)]))

    producer.flush()
