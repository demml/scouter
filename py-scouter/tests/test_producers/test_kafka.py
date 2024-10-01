from scouter import (
    KafkaConfig,
    KafkaProducer,
    SpcServerRecord,
    ServerRecords,
)
import pytest


def test_kafka_config():
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_err

    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": 600_000,
        "message.max.bytes": 2097164,
    }


def test_kafka_config_credentials(monkeypatch):
    monkeypatch.setenv("KAFKA_SASL_USERNAME", "test-username")
    monkeypatch.setenv("KAFKA_SASL_PASSWORD", "test-password")

    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_err
    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": 600_000,
        "message.max.bytes": 2097164,
        "sasl.mechanisms": "PLAIN",
        "sasl.password": "test-password",
        "sasl.username": "test-username",
        "security.protocol": "SASL_SSL",
    }


def test_kafka_producer(mock_kafka_producer):
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    producer = KafkaProducer(config)

    assert producer._kafka_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None

    record = SpcServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(ServerRecords(records=[record]))
    producer.flush()
    producer.flush(10)

    class Msg:
        def value(self):
            return "test"

        def topic(self):
            return "test-topic"

        def partition(self):
            return 0

        def offset(self):
            return 0

    with pytest.raises(ValueError):
        producer._delivery_report(err=202, msg=Msg(), raise_on_err=True)

    producer._delivery_report(err=None, msg=Msg(), raise_on_err=False)
