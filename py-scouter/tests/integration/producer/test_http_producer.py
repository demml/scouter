from scouter.client import HTTPConfig
from scouter.queue import (
    CustomMetricServerRecord,
    PsiServerRecord,
    ScouterProducer,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


def test_http_config(http_scouter_server):
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="guest",
        password="guest",
    )

    assert config.server_uri == "http://localhost:8000"
    assert config.username == "guest"
    assert config.password == "guest"


def test_http_producer_spc(http_scouter_server):
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="guest",
        password="guest",
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


def test_http_producer_psi(http_scouter_server):
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="guest",
        password="guest",
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


def test_http_producer_custom(http_scouter_server):
    config = HTTPConfig(
        server_uri="http://localhost:8000",
        username="guest",
        password="guest",
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
