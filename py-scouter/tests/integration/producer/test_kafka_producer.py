from scouter.queue import (
    CustomMetricServerRecord,
    KafkaConfig,
    PsiServerRecord,
    ScouterProducer,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


def test_kafka_config():
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_error=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_error

    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": "600000",
        "message.max.bytes": "2097164",
    }


def test_kafka_config_credentials(monkeypatch):
    monkeypatch.setenv("KAFKA_SASL_USERNAME", "test-username")
    monkeypatch.setenv("KAFKA_SASL_PASSWORD", "test-password")

    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_error=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_error
    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": "600000",
        "message.max.bytes": "2097164",
        "sasl.mechanisms": "PLAIN",
        "sasl.password": "test-password",
        "sasl.username": "test-username",
        "security.protocol": "SASL_SSL",
    }


def test_kafka_producer_spc(kafka_scouter_server):
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_error=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    producer = ScouterProducer(config)

    record = SpcServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_kafka_producer_psi(kafka_scouter_server):
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_error=True,
        config={"bootstrap.servers": "localhost:9092"},
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

    producer.publish(ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_kafka_producer_custom(kafka_scouter_server):
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_error=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    producer = ScouterProducer(config)

    record = CustomMetricServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        metric="metric",
        value=0.1,
    )

    producer.publish(ServerRecords(records=[ServerRecord(record)]))
    producer.flush()
